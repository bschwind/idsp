use crate::{NIBBLES_PER_FRAME, SAMPLES_PER_FRAME};

pub trait DivideByRoundUp {
    fn divide_by_round_up(&self, divisor: usize) -> usize;
    fn divide_by_2_round_up(&self) -> usize;
}

impl DivideByRoundUp for usize {
    fn divide_by_round_up(&self, divisor: usize) -> usize {
        (*self as f64 / divisor as f64).ceil() as usize
    }

    fn divide_by_2_round_up(&self) -> usize {
        (*self / 2) + (*self & 1)
    }
}

pub fn clamp_16(value: i32) -> i16 {
    if value > i16::MAX as i32 {
        return i16::MAX;
    }

    if value < i16::MIN as i32 {
        return i16::MIN;
    }

    value as i16
}

pub fn clamp_4(value: i32) -> i8 {
    if value > 7 {
        return 7;
    }

    if value < -8 {
        return -8;
    }

    value as i8
}

const SIGNED_NIBBLES: &[i8] = &[0, 1, 2, 3, 4, 5, 6, 7, -8, -7, -6, -5, -4, -3, -2, -1];

pub fn low_nibble(byte: u8) -> u8 {
    byte & 0xF
}

pub fn high_nibble(byte: u8) -> u8 {
    (byte >> 4) & 0xF
}

pub fn low_nibble_signed(byte: u8) -> i8 {
    SIGNED_NIBBLES[(byte & 0xF) as usize]
}

pub fn high_nibble_signed(byte: u8) -> i8 {
    SIGNED_NIBBLES[((byte >> 4) & 0xF) as usize]
}

pub fn combine_nibbles(high: i32, low: i32) -> u8 {
    ((high << 4) | (low & 0xF)) as u8
}

pub fn byte_count_to_sample_count(byte_count: usize) -> usize {
    nibble_count_to_sample_count(byte_count * 2)
}

pub fn sample_count_to_byte_count(sample_count: usize) -> usize {
    sample_count_to_nibble_count(sample_count).divide_by_2_round_up()
}

fn nibble_count_to_sample_count(nibble_count: usize) -> usize {
    let frames = nibble_count / NIBBLES_PER_FRAME;
    let extra_nibbles = nibble_count % NIBBLES_PER_FRAME;
    let extra_samples = if extra_nibbles < 2 { 0 } else { extra_nibbles - 2 };

    SAMPLES_PER_FRAME * frames + extra_samples
}

pub fn sample_count_to_nibble_count(sample_count: usize) -> usize {
    let frames = sample_count / SAMPLES_PER_FRAME;
    let extra_samples = sample_count % SAMPLES_PER_FRAME;
    let extra_nibbles = if extra_samples == 0 { 0 } else { extra_samples + 2 };

    NIBBLES_PER_FRAME * frames + extra_nibbles
}

pub fn get_next_multiple(value: usize, multiple: usize) -> usize {
    if multiple == 0 || value % multiple == 0 {
        value
    } else {
        value + multiple - value % multiple
    }
}
