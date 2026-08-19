use mirajazz::{
    device::DeviceQuery,
    types::{HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};

// Must be unique between all the plugins, 2 characters long and match `DeviceNamespace` field in `manifest.json`
pub const DEVICE_NAMESPACE: &str = "n1";

/// The N1 is a numpad replacement, so it stands in portrait: 15 LCD keys in 3 columns by
/// 5 rows, with a strip of three secondary screens above them. Verified on hardware --
/// pressing the top-right key reports 0x03 and the bottom-left key reports 0x0D.
///
/// We expose a 3x6 grid and give the secondary strip the last row, which lines up exactly:
/// the device's display slot is always the grid index plus one, for main and secondary keys
/// alike.
pub const ROW_COUNT: usize = 6;
pub const COL_COUNT: usize = 3;
pub const KEY_COUNT: usize = ROW_COUNT * COL_COUNT; // 18
pub const ENCODER_COUNT: usize = 1; // the rotary knob

/// Grid positions of the last row: the secondary 64x64 screens, not the main LCD keys.
/// Grid positions of the secondary strip: two 64x64 screens with a button behind each,
/// and a third screen with the knob under it. Physically this strip is the top row, so it
/// takes row 0 and the 15 main keys start below it -- otherwise OpenDeck draws the deck
/// upside down with respect to the hardware.
pub const SECONDARY_KEYS: [u8; 3] = [0, 1, 2];

/// The knob sits under the third screen of the strip, so its grid slot is the one no key
/// reports. OpenDeck draws the encoder there rather than in a row of its own.
pub const KNOB_POSITION: u16 = 2;

/// Hardware key codes, byte[9] of an `ACK..OK` input report.
/// Source: MiraboxSpace/StreamDock-Device-SDK, cross-checked against two physical units.
pub const HW_TOP_BUTTON_LEFT: u8 = 0x1E;
pub const HW_TOP_BUTTON_RIGHT: u8 = 0x1F;
pub const HW_KNOB_PRESS: u8 = 0x23;
pub const HW_KNOB_LEFT: u8 = 0x32;
pub const HW_KNOB_RIGHT: u8 = 0x33;
/// Not a control: the device echoes this after a successful image write.
pub const HW_WRITE_CONFIRM: u8 = 0xFF;

#[derive(Debug, Clone)]
pub enum Kind {
    VsdN1,
}

/// The USB stack reports this as "HOTSPOTEKUSB HID DEMO" regardless of the retail brand.
/// Sold as VSDinside / TreasLin Stream Dock N1, also badged ActionRing N1.
pub const HOTSPOTEK_VID: u16 = 0x5548;
pub const VSD_N1_PID: u16 = 0x1002;

pub const VSD_N1_QUERY: DeviceQuery = DeviceQuery::new(65440, 1, HOTSPOTEK_VID, VSD_N1_PID);

pub const QUERIES: [DeviceQuery; 1] = [VSD_N1_QUERY];

impl Kind {
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        match (vid, pid) {
            (HOTSPOTEK_VID, VSD_N1_PID) => Some(Kind::VsdN1),
            _ => None,
        }
    }

    /// There is no point relying on the names reported by the USB stack, so return our own
    pub fn human_name(&self) -> String {
        match &self {
            Self::VsdN1 => "VSD Stream Dock N1",
        }
        .to_string()
    }

    /// 1024-byte packets, unique per-unit serial, and both key edges reported => version 3
    pub fn protocol_version(&self) -> usize {
        3
    }

    pub fn supports_both_states(&self) -> bool {
        true
    }

    /// The N1 boots emulating a keyboard and reports nothing on the vendor interface until it
    /// is switched into software mode.
    ///
    /// Mind the value. The vendor SDK documents 2 as software mode, but on this PID 2 selects
    /// the calculator; 3 is software. Sending the documented 2 leaves the device connected and
    /// silent, which looks exactly like a broken reader.
    pub fn software_mode(&self) -> u8 {
        3
    }
}

// Verified on hardware: numbers pushed at Rot0 with no mirroring render upright and in the
// right cell. Kept as named constants because this is the one thing no descriptor tells you --
// a firmware revision that flips the panel would be fixed here and nowhere else.
pub const IMAGE_ROTATION: ImageRotation = ImageRotation::Rot0;
pub const IMAGE_MIRRORING: ImageMirroring = ImageMirroring::None;

/// Main LCD keys are 96x96, the secondary screens in the last column are 64x64.
pub fn get_image_format_for_key(_kind: &Kind, key: u8) -> ImageFormat {
    let size = if SECONDARY_KEYS.contains(&key) {
        (64, 64)
    } else {
        (96, 96)
    };

    ImageFormat {
        mode: ImageMode::JPEG,
        size,
        rotation: IMAGE_ROTATION,
        mirror: IMAGE_MIRRORING,
    }
}

#[derive(Debug, Clone)]
pub struct CandidateDevice {
    pub id: String,
    pub dev: HidDeviceInfo,
    pub kind: Kind,
}
