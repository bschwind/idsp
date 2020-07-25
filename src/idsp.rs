use crate::{sample_count_to_byte_count, DivideByRoundUp};
use bytes::{Buf, Bytes};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
};

const IDSP_HEADER: &[u8] = b"IDSP";

#[derive(Debug)]
pub enum DecodeError {
    Io(std::io::Error),
    InvalidHeader,
    InvalidAudioLength,
}

impl From<std::io::Error> for DecodeError {
    fn from(err: std::io::Error) -> Self {
        DecodeError::Io(err)
    }
}

#[derive(Debug)]
pub struct IdspContainer {
    looping: bool,
    channel_count: usize,
    sample_rate: usize,
    loop_start: usize,
    loop_end: usize,
    sample_count: usize,
    audio_data_offset: usize,
    interleave_size: usize,
    header_size: usize,
    channel_info_size: usize,
    audio_data_length: usize,
    channels: Vec<ChannelMetadata>,
    audio_data: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub struct ChannelMetadata {
    sample_count: usize,
    nibble_count: usize,
    sample_rate: usize,
    looping: bool,
    start_address: usize,
    end_address: usize,
    current_address: usize,
    coefficients: [i16; 16],
    gain: i16,
    start_context: GcAdpcmContext,
    loop_context: GcAdpcmContext,
}

#[derive(Debug)]
pub struct GcAdpcmContext {
    predictor_scale: i16,
    hist_1: i16,
    hist_2: i16,
}

impl GcAdpcmContext {
    pub fn read_from_buf(buf: &mut Cursor<Bytes>) -> Self {
        let predictor_scale = buf.get_i16();
        let hist_1 = buf.get_i16();
        let hist_2 = buf.get_i16();

        Self { predictor_scale, hist_1, hist_2 }
    }
}

pub fn read_idsp<P: AsRef<Path>>(file_path: P) -> Result<IdspContainer, DecodeError> {
    let mut file = File::open(file_path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    read_idsp_bytes(&bytes)
}

pub fn read_idsp_bytes(original_bytes: &[u8]) -> Result<IdspContainer, DecodeError> {
    let mut bytes = Cursor::new(Bytes::copy_from_slice(original_bytes));

    if &Buf::bytes(&bytes)[..IDSP_HEADER.len()] != IDSP_HEADER {
        return Err(DecodeError::InvalidHeader);
    }

    bytes.advance(IDSP_HEADER.len());
    bytes.advance(4); // Skip this empty space?

    let channel_count = bytes.get_i32() as usize;
    let sample_rate = bytes.get_i32() as usize;
    let sample_count = bytes.get_i32() as usize;
    let loop_start = bytes.get_i32() as usize;
    let loop_end = bytes.get_i32() as usize;
    let interleave_size = bytes.get_i32() as usize;
    let header_size = bytes.get_i32() as usize;
    let channel_info_size = bytes.get_i32() as usize;
    let audio_data_offset = bytes.get_i32() as usize;
    let audio_data_length = bytes.get_i32() as usize;

    let mut channels = vec![];
    for i in 0..channel_count {
        bytes.set_position((header_size + i * channel_info_size) as u64);

        let sample_count = bytes.get_i32() as usize;
        let nibble_count = bytes.get_i32() as usize;
        let sample_rate = bytes.get_i32() as usize;
        let looping = bytes.get_i16() == 1;
        bytes.advance(2);
        let start_address = bytes.get_i32() as usize;
        let end_address = bytes.get_i32() as usize;
        let current_address = bytes.get_i32() as usize;
        let mut coefficients = [0; 16];

        for c in &mut coefficients {
            *c = bytes.get_i16();
        }

        let gain = bytes.get_i16();
        let start_context = GcAdpcmContext::read_from_buf(&mut bytes);
        let loop_context = GcAdpcmContext::read_from_buf(&mut bytes);

        let channel = ChannelMetadata {
            sample_count,
            nibble_count,
            sample_rate,
            looping,
            start_address,
            end_address,
            current_address,
            coefficients,
            gain,
            start_context,
            loop_context,
        };

        channels.push(channel);
    }

    let looping = channels.iter().any(|c| c.looping);

    // Read audio data
    bytes.set_position(audio_data_offset as u64);
    let interleave: usize = if interleave_size == 0 { audio_data_length } else { interleave_size };

    let audio_data = deinterleave(
        &mut bytes,
        channel_count * audio_data_length,
        interleave,
        channel_count,
        Some(sample_count_to_byte_count(sample_count)),
    )?;

    let container = IdspContainer {
        looping,
        channel_count,
        sample_rate,
        sample_count,
        loop_start,
        loop_end,
        interleave_size,
        header_size,
        channel_info_size,
        audio_data_offset,
        audio_data_length,
        channels,
        audio_data,
    };

    Ok(container)
}

fn deinterleave(
    bytes: &mut Cursor<Bytes>,
    len: usize,
    interleave_size: usize,
    output_count: usize,
    output_size: Option<usize>,
) -> Result<Vec<Vec<u8>>, DecodeError> {
    let remaining_length = bytes.get_ref().len() - bytes.position() as usize;

    if remaining_length < len {
        // Specified length is greater than the number of bytes remaining in the Stream
        return Err(DecodeError::InvalidAudioLength);
    }

    if len % output_count != 0 {
        // The input length must be divisible by the number of outputs.
        return Err(DecodeError::InvalidAudioLength);
    }

    let input_size = len / output_count;
    let output_size = output_size.unwrap_or(input_size);

    let in_block_count = input_size.divide_by_round_up(interleave_size);
    let out_block_count = output_size.divide_by_round_up(interleave_size);
    let last_input_interleave_size = input_size - (in_block_count - 1) * interleave_size;
    let last_output_interleave_size = output_size - (out_block_count - 1) * interleave_size;
    let blocks_to_copy = in_block_count.min(out_block_count);

    let mut outputs = vec![vec![0; output_size]; output_count];

    for b in 0..blocks_to_copy {
        let current_input_interlave_size =
            if b == in_block_count - 1 { last_input_interleave_size } else { interleave_size };

        let current_output_interleave_size =
            if b == out_block_count - 1 { last_output_interleave_size } else { interleave_size };

        let bytes_to_copy = current_input_interlave_size.min(current_output_interleave_size);

        for o in 0..output_count {
            let offset = interleave_size * b;
            bytes.copy_to_slice(&mut outputs[o][offset..(offset + bytes_to_copy)]);

            if bytes_to_copy < current_input_interlave_size {
                bytes.advance(current_input_interlave_size - bytes_to_copy);
            }
        }
    }

    Ok(outputs)
}

mod test {
    use super::*;
    use crate::decode_gc_adpcm;
    use wav::{BitDepth, Header};

    #[test]
    fn test_file_read() {
        let idsp_bytes = include_bytes!("../test_files/13.idsp");
        let _idsp_file = read_idsp_bytes(idsp_bytes);
    }

    #[test]
    fn test_file_decode() {
        let idsp_bytes = include_bytes!("../test_files/13.idsp");
        let idsp_file = read_idsp_bytes(idsp_bytes).unwrap();

        assert_eq!(idsp_file.channels.len(), 1);
        assert_eq!(idsp_file.audio_data.len(), 1);

        let decoded: Vec<i16> =
            decode_gc_adpcm(&idsp_file.audio_data[0], &idsp_file.channels[0].coefficients);

        let header =
            Header::new(1, idsp_file.channels.len() as u16, idsp_file.sample_rate as u32, 16);

        let mut output_file = std::fs::File::create("lol.wav").unwrap();
        wav::write(header, BitDepth::Sixteen(decoded), &mut output_file).unwrap();
    }
}
