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

/// Converts a hardware key code into an index in OpenDeck's row-major 3-column grid.
///
/// The 15 LCD keys arrive as 0x01..=0x0F in reading order and fill rows 0..4 exactly, so the
/// mapping is a plain subtract-one. The two physical buttons of the secondary strip take the
/// first two slots of the last row; the third secondary screen has no button behind it.
pub fn device_to_opendeck(input: u8) -> Option<usize> {
    match input {
        1..=LCD_KEY_COUNT => Some((input - 1) as usize),
        HW_TOP_BUTTON_LEFT => Some(15),
        HW_TOP_BUTTON_RIGHT => Some(16),
        _ => None,
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
        assert_eq!(device_to_opendeck(0x01), Some(0), "top-left");
        assert_eq!(device_to_opendeck(0x03), Some(2), "top-right");
        assert_eq!(device_to_opendeck(0x0D), Some(12), "bottom-left");
        assert_eq!(device_to_opendeck(0x0F), Some(14), "bottom-right");

        // The LCD block never spills into the secondary row
        for code in 1..=LCD_KEY_COUNT {
            let index = device_to_opendeck(code).unwrap();
            assert!(index < 15, "key {code:#04x} spilled into the secondary row");
        }
    }

    #[test]
    fn top_buttons_take_the_last_row_and_nothing_else_maps() {
        assert_eq!(device_to_opendeck(HW_TOP_BUTTON_LEFT), Some(15));
        assert_eq!(device_to_opendeck(HW_TOP_BUTTON_RIGHT), Some(16));
        assert!(KEY_COUNT == 18);

        // Knob codes are handled as encoder events, never as buttons
        assert_eq!(device_to_opendeck(HW_KNOB_PRESS), None);
        assert_eq!(device_to_opendeck(HW_KNOB_LEFT), None);
        assert_eq!(device_to_opendeck(HW_KNOB_RIGHT), None);
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
