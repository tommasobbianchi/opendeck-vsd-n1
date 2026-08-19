use mirajazz::{error::MirajazzError, types::DeviceInput};

use crate::mappings::{
    ENCODER_COUNT, HW_KNOB_LEFT, HW_KNOB_PRESS, HW_KNOB_RIGHT, HW_TOP_BUTTON_LEFT,
    HW_TOP_BUTTON_RIGHT, HW_WRITE_CONFIRM, KEY_COUNT,
};

/// Number of main LCD keys, reported by the device as 0x01..=0x0F
const LCD_KEY_COUNT: u8 = 15;

pub fn process_input(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    log::debug!("Processing input: {:#04x}, {}", input, state);

    match input {
        // A zero key code means "nothing pressed", used to resynchronise state
        0 => Ok(DeviceInput::ButtonStateChange(vec![false; KEY_COUNT])),
        // Every image write is echoed back on the same endpoint. It is not a control, and
        // treating it as one turns a normal repaint into a stream of BadData errors.
        HW_WRITE_CONFIRM => Ok(DeviceInput::NoData),
        HW_KNOB_LEFT | HW_KNOB_RIGHT => read_encoder_value(input),
        HW_KNOB_PRESS => read_encoder_press(state),
        _ => read_button_press(input, state),
    }
}

/// Number of slots in the secondary strip, which occupies grid row 0.
const SECONDARY_ROW: usize = 3;

/// Converts a hardware key code into an index in OpenDeck's row-major 3-column grid.
///
/// The strip of small screens is physically *above* the 15 main keys, so it takes row 0 and
/// the main block starts at row 1. Getting this backwards is not cosmetic: OpenDeck lays keys
/// out in grid order, so the on-screen deck would be a mirror of the one under your hands.
///
/// Its two buttons take the first two slots; the third slot is the screen the knob sits under,
/// which has no button and so never reports a key code.
pub fn device_to_opendeck(input: u8) -> Option<usize> {
    match input {
        1..=LCD_KEY_COUNT => Some(SECONDARY_ROW + (input - 1) as usize),
        HW_TOP_BUTTON_LEFT => Some(0),
        HW_TOP_BUTTON_RIGHT => Some(1),
        _ => None,
    }
}

/// Inverse of [`device_to_opendeck`] for image writes.
///
/// mirajazz addresses screens as `key + 1` on the wire, and the device numbers the main block
/// 1..=15 before the secondary strip 16..=18 -- the opposite order to the grid above. Without
/// this translation the images land on the right device but the wrong screens.
pub fn opendeck_to_device(position: u8) -> u8 {
    if (position as usize) < SECONDARY_ROW {
        LCD_KEY_COUNT + position
    } else {
        position - SECONDARY_ROW as u8
    }
}

fn read_button_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    let index = device_to_opendeck(input).ok_or(MirajazzError::BadData)?;

    // ponytail: one report carries one key, so we rebuild the whole vector with a single
    // key set. Holding two keys at once therefore reports the older one as released. Same
    // behaviour as the upstream akp03/akp153 plugins; fix it here if chording ever matters.
    let mut states = vec![false; KEY_COUNT];
    states[index] = state != 0;

    Ok(DeviceInput::ButtonStateChange(states))
}

fn read_encoder_value(input: u8) -> Result<DeviceInput, MirajazzError> {
    let mut values = vec![0i8; ENCODER_COUNT];

    values[0] = match input {
        HW_KNOB_LEFT => -1,
        HW_KNOB_RIGHT => 1,
        _ => return Err(MirajazzError::BadData),
    };

    Ok(DeviceInput::EncoderTwist(values))
}

fn read_encoder_press(state: u8) -> Result<DeviceInput, MirajazzError> {
    let mut states = vec![false; ENCODER_COUNT];
    states[0] = state != 0;

    Ok(DeviceInput::EncoderStateChange(states))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcd_corners_match_what_the_hardware_reported() {
        // Measured on the device: pressing each corner of the 3x5 block produced these codes.
        // They start at grid 3 because the secondary strip sits above them, in row 0.
        assert_eq!(device_to_opendeck(0x01), Some(3), "top-left");
        assert_eq!(device_to_opendeck(0x03), Some(5), "top-right");
        assert_eq!(device_to_opendeck(0x0D), Some(15), "bottom-left");
        assert_eq!(device_to_opendeck(0x0F), Some(17), "bottom-right");

        // The LCD block never spills into the secondary row
        for code in 1..=LCD_KEY_COUNT {
            let index = device_to_opendeck(code).unwrap();
            assert!(index >= 3, "key {code:#04x} spilled into the secondary row");
        }
    }

    #[test]
    fn top_buttons_take_the_first_row_and_nothing_else_maps() {
        assert_eq!(device_to_opendeck(HW_TOP_BUTTON_LEFT), Some(0));
        assert_eq!(device_to_opendeck(HW_TOP_BUTTON_RIGHT), Some(1));
        assert!(KEY_COUNT == 18);

        // Knob codes are handled as encoder events, never as buttons
        assert_eq!(device_to_opendeck(HW_KNOB_PRESS), None);
        assert_eq!(device_to_opendeck(HW_KNOB_LEFT), None);
        assert_eq!(device_to_opendeck(HW_KNOB_RIGHT), None);
    }

    #[test]
    fn image_positions_round_trip_back_to_the_key_that_reports_them() {
        // Every screen with a button behind it must take its image on the same physical key
        // that reports the press, or the deck shows one thing and does another.
        for code in (1..=LCD_KEY_COUNT).chain([HW_TOP_BUTTON_LEFT, HW_TOP_BUTTON_RIGHT]) {
            let position = device_to_opendeck(code).unwrap() as u8;
            let wire_key = opendeck_to_device(position) + 1; // mirajazz sends key + 1
            let expected = if code >= HW_TOP_BUTTON_LEFT {
                LCD_KEY_COUNT + 1 + (code - HW_TOP_BUTTON_LEFT)
            } else {
                code
            };
            assert_eq!(wire_key, expected, "grid {position} for code {code:#04x}");
        }

        // The knob's screen has no button, so it is only ever addressed as an image.
        assert_eq!(opendeck_to_device(2) + 1, 18);
    }

    #[test]
    fn knob_rotation_and_press_produce_encoder_events() {
        match process_input(HW_KNOB_RIGHT, 0).unwrap() {
            DeviceInput::EncoderTwist(v) => assert_eq!(v, vec![1i8]),
            other => panic!("expected a twist, got {other:?}"),
        }
        match process_input(HW_KNOB_LEFT, 0).unwrap() {
            DeviceInput::EncoderTwist(v) => assert_eq!(v, vec![-1i8]),
            other => panic!("expected a twist, got {other:?}"),
        }
        match process_input(HW_KNOB_PRESS, 1).unwrap() {
            DeviceInput::EncoderStateChange(v) => assert_eq!(v, vec![true]),
            other => panic!("expected an encoder press, got {other:?}"),
        }
    }

    #[test]
    fn write_confirmations_are_ignored_rather_than_erroring() {
        assert!(matches!(
            process_input(HW_WRITE_CONFIRM, 0).unwrap(),
            DeviceInput::NoData
        ));
    }

    #[test]
    fn unknown_codes_are_rejected_rather_than_mapped_to_key_zero() {
        assert!(process_input(0x7F, 1).is_err());
    }
}
