pub mod decode;
pub mod encode;
pub mod idsp;
pub mod math;

use math::DivideByRoundUp;

const SAMPLES_PER_FRAME: usize = 14;
const NIBBLES_PER_FRAME: usize = 16;
const BYTES_PER_FRAME: usize = 8;

struct CodecParameters {
    sample_count: usize,
    history_1: i16,
    history_2: i16,
}

fn clamp_16(value: i32) -> i16 {
    if value > i16::MAX as i32 {
        return i16::MAX;
    }

    if value < i16::MIN as i32 {
        return i16::MIN;
    }

    value as i16
}

fn clamp_4(value: i32) -> i8 {
    if value > 7 {
        return 7;
    }

    if value < -8 {
        return -8;
    }

    value as i8
}

const SIGNED_NIBBLES: &[i8] = &[0, 1, 2, 3, 4, 5, 6, 7, -8, -7, -6, -5, -4, -3, -2, -1];

fn low_nibble(byte: u8) -> u8 {
    byte & 0xF
}

fn high_nibble(byte: u8) -> u8 {
    (byte >> 4) & 0xF
}

fn low_nibble_signed(byte: u8) -> i8 {
    SIGNED_NIBBLES[(byte & 0xF) as usize]
}

fn high_nibble_signed(byte: u8) -> i8 {
    SIGNED_NIBBLES[((byte >> 4) & 0xF) as usize]
}

fn combine_nibbles(high: i32, low: i32) -> u8 {
    ((high << 4) | (low & 0xF)) as u8
}

fn byte_count_to_sample_count(byte_count: usize) -> usize {
    nibble_count_to_sample_count(byte_count * 2)
}

fn sample_count_to_byte_count(sample_count: usize) -> usize {
    sample_count_to_nibble_count(sample_count).divide_by_2_round_up()
}

fn nibble_count_to_sample_count(nibble_count: usize) -> usize {
    let frames = nibble_count / NIBBLES_PER_FRAME;
    let extra_nibbles = nibble_count % NIBBLES_PER_FRAME;
    let extra_samples = if extra_nibbles < 2 { 0 } else { extra_nibbles - 2 };

    SAMPLES_PER_FRAME * frames + extra_samples
}

fn sample_count_to_nibble_count(sample_count: usize) -> usize {
    let frames = sample_count / SAMPLES_PER_FRAME;
    let extra_samples = sample_count % SAMPLES_PER_FRAME;
    let extra_nibbles = if extra_samples == 0 { 0 } else { extra_samples + 2 };

    NIBBLES_PER_FRAME * frames + extra_nibbles
}

fn get_next_multiple(value: usize, multiple: usize) -> usize {
    if multiple == 0 || value % multiple == 0 {
        value
    } else {
        value + multiple - value % multiple
    }
}
