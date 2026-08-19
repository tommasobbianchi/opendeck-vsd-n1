use mirajazz::{error::MirajazzError, types::DeviceInput};

use crate::mappings::{
    COL_COUNT, ENCODER_COUNT, HW_KNOB_LEFT, HW_KNOB_PRESS, HW_KNOB_RIGHT, HW_TOP_BUTTON_LEFT,
    HW_TOP_BUTTON_RIGHT, KEY_COUNT,
};

/// Number of main LCD keys, reported by the device as 0x01..=0x0F
const LCD_KEY_COUNT: u8 = 15;
/// The LCD block is 5 columns wide inside the 6-column OpenDeck grid
const LCD_COLS: usize = 5;

pub fn process_input(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    log::debug!("Processing input: {:#04x}, {}", input, state);

    match input {
        // A zero key code means "nothing pressed", used to resynchronise state
        0 => Ok(DeviceInput::ButtonStateChange(vec![false; KEY_COUNT])),
        HW_KNOB_LEFT | HW_KNOB_RIGHT => read_encoder_value(input),
        HW_KNOB_PRESS => read_encoder_press(state),
        _ => read_button_press(input, state),
    }
}

/// Converts a hardware key code into an index in OpenDeck's row-major 3x6 grid.
///
/// The 15 LCD keys arrive as 0x01..=0x0F in reading order and occupy columns 0..4.
/// The two physical top buttons are folded into the sixth column.
pub fn device_to_opendeck(input: u8) -> Option<usize> {
    match input {
        1..=LCD_KEY_COUNT => {
            let i = (input - 1) as usize;
            Some((i / LCD_COLS) * COL_COUNT + (i % LCD_COLS))
        }
        HW_TOP_BUTTON_LEFT => Some(5),
        HW_TOP_BUTTON_RIGHT => Some(11),
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
    fn lcd_keys_land_in_the_first_five_columns_in_reading_order() {
        // Row 0 of the device maps to grid 0..4, row 1 to 6..10, row 2 to 12..16.
        assert_eq!(device_to_opendeck(0x01), Some(0));
        assert_eq!(device_to_opendeck(0x05), Some(4));
        assert_eq!(device_to_opendeck(0x06), Some(6));
        assert_eq!(device_to_opendeck(0x0A), Some(10));
        assert_eq!(device_to_opendeck(0x0B), Some(12));
        assert_eq!(device_to_opendeck(0x0F), Some(16));

        // The sixth column is never used by an LCD key
        for code in 1..=LCD_KEY_COUNT {
            let index = device_to_opendeck(code).unwrap();
            assert_ne!(index % COL_COUNT, 5, "key {code:#04x} collided with column 6");
            assert!(index < KEY_COUNT);
        }
    }

    #[test]
    fn top_buttons_take_the_sixth_column_and_nothing_else_maps() {
        assert_eq!(device_to_opendeck(HW_TOP_BUTTON_LEFT), Some(5));
        assert_eq!(device_to_opendeck(HW_TOP_BUTTON_RIGHT), Some(11));

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
    fn unknown_codes_are_rejected_rather_than_mapped_to_key_zero() {
        assert!(process_input(0x7F, 1).is_err());
    }
}
