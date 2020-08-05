pub mod coefficients;
pub mod decode;
pub mod encode;
pub mod idsp;
pub mod math;

pub use crate::{
    coefficients::Coefficients,
    decode::decode_gc_adpcm,
    encode::encode_gc_adpcm,
    idsp::{read_idsp_bytes, write_idsp_bytes, IdspContainer},
};

const SAMPLES_PER_FRAME: usize = 14;
const NIBBLES_PER_FRAME: usize = 16;
const BYTES_PER_FRAME: usize = 8;

struct CodecParameters {
    sample_count: usize,
    history_1: i16,
    history_2: i16,
}

#[cfg(test)]
mod test {
    use crate::{
        coefficients::Coefficients,
        decode::decode_gc_adpcm,
        encode::encode_gc_adpcm,
        idsp::{read_idsp_bytes, write_idsp_bytes, IdspContainer},
    };

    #[test]
    fn full_roundtrip_test() {
        let idsp_bytes = include_bytes!("../test_files/13.idsp");
        let idsp_file = read_idsp_bytes(idsp_bytes).unwrap();

        assert_eq!(idsp_file.channels.len(), 1);

        let wav_pcm: Vec<i16> = decode_gc_adpcm(
            &idsp_file.channels[0].audio,
            &idsp_file.channels[0].metadata.coefficients,
        );

        let coefficients = Coefficients::from(&wav_pcm);

        let gcadpcm = encode_gc_adpcm(&wav_pcm, &*coefficients);

        write_idsp_bytes(&IdspContainer { sample_count: gcadpcm.sample_count, ..idsp_file })
            .unwrap();
    }
}
