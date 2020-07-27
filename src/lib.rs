pub mod coefficients;
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
