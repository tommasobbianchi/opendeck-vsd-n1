use mirajazz::{
    device::DeviceQuery,
    types::{HidDeviceInfo, ImageFormat, ImageMirroring, ImageMode, ImageRotation},
};

// Must be unique between all the plugins, 2 characters long and match `DeviceNamespace` field in `manifest.json`
pub const DEVICE_NAMESPACE: &str = "n1";

/// The N1 has 15 LCD keys in a 3x5 block plus a strip of secondary screens and two
/// physical buttons. OpenDeck can only register a rectangular grid, so we expose 3x6 and
/// fold the non-LCD controls into the sixth column -- the same compromise opendeck-akp153
/// makes for its side buttons.
pub const ROW_COUNT: usize = 3;
pub const COL_COUNT: usize = 6;
pub const KEY_COUNT: usize = ROW_COUNT * COL_COUNT; // 18
pub const ENCODER_COUNT: usize = 1; // the rotary knob

/// Grid positions of the sixth column: the secondary 64x64 screens, not the main LCD keys.
pub const SECONDARY_KEYS: [u8; 3] = [5, 11, 17];

/// Hardware key codes, byte[9] of an `ACK..OK` input report.
/// Source: MiraboxSpace/StreamDock-Device-SDK, cross-checked against two physical units.
pub const HW_TOP_BUTTON_LEFT: u8 = 0x1E;
pub const HW_TOP_BUTTON_RIGHT: u8 = 0x1F;
pub const HW_KNOB_PRESS: u8 = 0x23;
pub const HW_KNOB_LEFT: u8 = 0x32;
pub const HW_KNOB_RIGHT: u8 = 0x33;

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
}

// ponytail: rotation and mirroring are the one thing no descriptor tells you -- they have to
// be seen on the glass. Rot0/None is what lightslinger sends to this exact PID and it renders
// upright, so start there. If icons come out turned or flipped, these two constants are the
// only knob you need to touch.
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
