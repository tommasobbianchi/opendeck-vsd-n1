use data_url::DataUrl;
use image::load_from_memory_with_format;
use mirajazz::{device::Device, error::MirajazzError, state::DeviceStateUpdate};
use openaction::{OUTBOUND_EVENT_MANAGER, SetImageEvent};
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS,
    mappings::{
        COL_COUNT, CandidateDevice, ENCODER_COUNT, KEY_COUNT, Kind, ROW_COUNT,
        get_image_format_for_key,
    },
};

/// Initializes a device and listens for events
pub async fn device_task(candidate: CandidateDevice, token: CancellationToken) {
    log::info!("Running device task for {:?}", candidate);

    // Wrap in a closure so we can use `?` operator
    let device = async || -> Result<Device, MirajazzError> {
        let device = connect(&candidate).await?;

        // Must come first: until the device is in software mode it reports no input at all.
        device.set_mode(candidate.kind.software_mode()).await?;
        device.set_brightness(50).await?;
        device.clear_all_button_images().await?;
        device.flush().await?;

        Ok(device)
    }()
    .await;

    let device: Device = match device {
        Ok(device) => device,
        Err(err) => {
            handle_error(&candidate.id, err).await;

            log::error!(
                "Had error during device init, finishing device task: {:?}",
                candidate
            );

            return;
        }
    };

    announce(&candidate).await;

    DEVICES.write().await.insert(candidate.id.clone(), device);

    // A device that is already plugged in registers about a second after launch, while
    // OpenDeck's webview is still mounting: its initial get_devices returns empty and the
    // "devices" event fires before the listener is attached, so the UI sits on "No devices
    // detected" even though the backend holds the device. Re-announcing once the window has
    // settled costs nothing and makes a cold start behave like a hotplug.
    {
        let candidate = candidate.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(6)).await;
            announce(&candidate).await;
        });
    }

    tokio::select! {
        _ = device_events_task(&candidate) => {},
        _ = heartbeat_task(&candidate) => {},
        _ = token.cancelled() => {}
    };

    log::info!("Shutting down device {:?}", candidate);

    if let Some(device) = DEVICES.read().await.get(&candidate.id) {
        device.shutdown().await.ok();
    }

    log::info!("Device task finished for {:?}", candidate);
}

/// Tells OpenDeck about the device. Safe to call more than once: OpenDeck keys devices by id,
/// so a repeat registration refreshes the UI rather than creating a duplicate.
async fn announce(candidate: &CandidateDevice) {
    log::info!("Registering device {}", candidate.id);

    let mut manager = OUTBOUND_EVENT_MANAGER.lock().await;
    let Some(outbound) = manager.as_mut() else {
        log::error!("OUTBOUND_EVENT_MANAGER is None -- cannot register {}", candidate.id);
        return;
    };

    match outbound
        .register_device(
            candidate.id.clone(),
            candidate.kind.human_name(),
            ROW_COUNT as u8,
            COL_COUNT as u8,
            ENCODER_COUNT as u8,
            0,
        )
        .await
    {
        Ok(()) => log::info!("register_device event sent for {}", candidate.id),
        Err(e) => log::error!("register_device failed for {}: {e}", candidate.id),
    }
}

/// Handles errors, returning true if should continue, returning false if an error is fatal
pub async fn handle_error(id: &String, err: MirajazzError) -> bool {
    log::error!("Device {} error: {}", id, err);

    // Some errors are not critical and can be ignored without sending disconnected event
    if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
        return true;
    }

    log::info!("Deregistering device {}", id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        outbound.deregister_device(id.clone()).await.unwrap();
    }

    log::info!("Cancelling tasks for device {}", id);
    if let Some(token) = TOKENS.read().await.get(id) {
        token.cancel();
    }

    log::info!("Removing device {} from the list", id);
    DEVICES.write().await.remove(id);

    log::info!("Finished clean-up for {}", id);

    false
}

pub async fn connect(candidate: &CandidateDevice) -> Result<Device, MirajazzError> {
    let firmware_version = Device::read_firmware_version(&candidate.dev).await;

    let firmware_version = match firmware_version {
        Ok(fw) => fw,
        Err(e) => {
            log::error!("Failed to read firmware version from {}", &candidate.id);

            return Err(e);
        }
    };

    log::info!(
        "Connecting to {} with fw {:?}",
        &candidate.id,
        &firmware_version
    );

    let result = Device::connect(
        &candidate.dev,
        candidate.kind.protocol_version(),
        KEY_COUNT,
        ENCODER_COUNT,
    )
    .await;

    match result {
        // The N1 reports both edges for keys and for the knob press, so let mirajazz
        // surface release events instead of synthesising them.
        Ok(device) => Ok(device
            .with_supports_both_keypress_states(candidate.kind.supports_both_states())
            .with_supports_both_encoder_states(candidate.kind.supports_both_states())),
        Err(e) => {
            log::error!("Error while connecting to device: {e}");

            Err(e)
        }
    }
}

/// The N1 drops the host and re-enumerates after roughly 35 seconds of silence, so it needs a
/// periodic `CRT..CONNECT`. Devices in the AKP03/N3 family do not, which is why the upstream
/// plugin never calls `keep_alive`. The vendor SDK uses a 10s period; we halve it so a single
/// lost write is not enough to drop the device.
async fn heartbeat_task(candidate: &CandidateDevice) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
    ticker.tick().await; // the first tick completes immediately

    loop {
        ticker.tick().await;

        let result = {
            let devices = DEVICES.read().await;
            match devices.get(&candidate.id) {
                Some(device) => device.keep_alive().await,
                None => return, // device went away, nothing left to keep alive
            }
            // read guard is dropped here, before any handle_error takes a write lock
        };

        if let Err(e) = result {
            log::error!("Heartbeat failed for {}: {}", candidate.id, e);
            handle_error(&candidate.id, e).await;
            return;
        }
    }
}

/// Handles events from device to OpenDeck
async fn device_events_task(candidate: &CandidateDevice) -> Result<(), MirajazzError> {
    log::info!("Connecting to {} for incoming events", candidate.id);

    let devices_lock = DEVICES.read().await;
    let reader = match devices_lock.get(&candidate.id) {
        Some(device) => device.get_reader(crate::inputs::process_input),
        None => return Ok(()),
    };
    drop(devices_lock);

    // mirajazz starts DeviceState with empty vectors, and its diff loop zips ours against
    // them. zip stops at the shorter side, so the first report after connecting yields no
    // updates at all and only establishes the baseline -- the first key you press is silently
    // swallowed. Priming the vectors to the right length makes that first press count.
    {
        let mut states = reader.states.lock().await;
        states.buttons = vec![false; KEY_COUNT];
        states.encoders = vec![false; ENCODER_COUNT];
    }

    log::info!("Connected to {} for incoming events", candidate.id);

    log::info!("Reader is ready for {}", candidate.id);

    loop {
        log::info!("Reading updates...");

        let updates = match reader.read(None).await {
            Ok(updates) => updates,
            Err(e) => {
                if !handle_error(&candidate.id, e).await {
                    break;
                }

                continue;
            }
        };

        for update in updates {
            log::info!("New update: {:#?}", update);

            let id = candidate.id.clone();

            if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                match update {
                    DeviceStateUpdate::ButtonDown(key) => outbound.key_down(id, key).await.unwrap(),
                    DeviceStateUpdate::ButtonUp(key) => outbound.key_up(id, key).await.unwrap(),
                    DeviceStateUpdate::EncoderDown(encoder) => {
                        outbound.encoder_down(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderUp(encoder) => {
                        outbound.encoder_up(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderTwist(encoder, val) => {
                        outbound
                            .encoder_change(id, encoder, val as i16)
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handles different combinations of "set image" event, including clearing the specific buttons and whole device
pub async fn handle_set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
    match (evt.position, evt.image) {
        (Some(position), Some(image)) => {
            log::info!("Setting image for button {}", position);

            // OpenDeck sends image as a data url, so parse it using a library
            let url = DataUrl::process(image.as_str()).unwrap(); // Isn't expected to fail, so unwrap it is
            let (body, _fragment) = url.decode_to_vec().unwrap(); // Same here

            // Allow only image/jpeg mime for now
            if url.mime_type().subtype != "jpeg" {
                log::error!("Incorrect mime type: {}", url.mime_type());

                return Ok(()); // Not a fatal error, enough to just log it
            }

            let image = load_from_memory_with_format(body.as_slice(), image::ImageFormat::Jpeg)?;

            device
                .set_button_image(
                    position,
                    get_image_format_for_key(
                        &Kind::from_vid_pid(device.vid, device.pid).unwrap(),
                        position,
                    ),
                    image,
                )
                .await?;
            device.flush().await?;
        }
        (Some(position), None) => {
            device.clear_button_image(position).await?;
            device.flush().await?;
        }
        (None, None) => {
            device.clear_all_button_images().await?;
            device.flush().await?;
        }
        _ => {}
    }

    Ok(())
}
