use crate::{
    math::{clamp_16, clamp_4, combine_nibbles, sample_count_to_byte_count, DivideByRoundUp},
    CodecParameters, BYTES_PER_FRAME, SAMPLES_PER_FRAME,
};

struct AdpcmEncodeBuffers {
    coefficients: Vec<Vec<i16>>,
    pcm_out: Vec<Vec<i32>>,
    adpcm_out: Vec<Vec<i32>>,
    scale: Vec<i32>,
    total_distance: Vec<f64>,
}

impl AdpcmEncodeBuffers {
    fn new() -> Self {
        let mut coefficients = vec![vec![]; 8];
        let mut pcm_out = vec![vec![]; 8];
        let mut adpcm_out = vec![vec![]; 8];
        let scale = vec![0; 8];
        let total_distance = vec![0.0; 8];

        for i in 0..8 {
            pcm_out[i] = vec![0; 16];
            adpcm_out[i] = vec![0; 14];
            coefficients[i] = vec![0; 2];
        }

        Self { coefficients, pcm_out, adpcm_out, scale, total_distance }
    }
}

pub fn encode_gc_adpcm(pcm: &[i16], coefficients: &[i16]) -> Vec<u8> {
    let config = CodecParameters { sample_count: pcm.len(), history_1: 0, history_2: 0 };

    let sample_count = pcm.len();
    let mut adpcm = vec![0; sample_count_to_byte_count(sample_count)];

    let mut pcm_buffer = vec![0i16; 2 + SAMPLES_PER_FRAME];
    let mut adpcm_buffer = vec![0u8; BYTES_PER_FRAME];

    pcm_buffer[0] = config.history_2;
    pcm_buffer[1] = config.history_1;

    let frame_count = sample_count.divide_by_round_up(SAMPLES_PER_FRAME);
    let mut buffers = AdpcmEncodeBuffers::new();

    for frame in 0..frame_count {
        let samples_to_copy = (sample_count - frame * SAMPLES_PER_FRAME).min(SAMPLES_PER_FRAME);
        let src_index = frame * SAMPLES_PER_FRAME;
        pcm_buffer[2..(2 + samples_to_copy)]
            .copy_from_slice(&pcm[src_index..(src_index + samples_to_copy)]);

        let amount_to_clear = SAMPLES_PER_FRAME - samples_to_copy;
        let clear_start = 2 + samples_to_copy;
        for sample in pcm_buffer[clear_start..(clear_start + amount_to_clear)].iter_mut() {
            *sample = 0;
        }

        dsp_encode_frame(
            &mut pcm_buffer,
            SAMPLES_PER_FRAME,
            &mut adpcm_buffer,
            &coefficients,
            &mut buffers,
        );
        let bytes_to_copy = sample_count_to_byte_count(samples_to_copy);
        let dst_index = frame * BYTES_PER_FRAME;
        adpcm[dst_index..(dst_index + bytes_to_copy)]
            .copy_from_slice(&adpcm_buffer[0..bytes_to_copy]);

        pcm_buffer[0] = pcm_buffer[14];
        pcm_buffer[1] = pcm_buffer[15];
    }

    adpcm
}

fn dsp_encode_frame(
    pcm_in_out: &mut [i16],
    sample_count: usize,
    adpcm_out: &mut [u8],
    coefficients_in: &[i16],
    b: &mut AdpcmEncodeBuffers,
) {
    for i in 0..8 {
        b.coefficients[i][0] = coefficients_in[i * 2];
        b.coefficients[i][1] = coefficients_in[i * 2 + 1];
    }

    for i in 0..8 {
        dsp_encode_coefficient(
            pcm_in_out,
            sample_count,
            &b.coefficients[i],
            &mut b.pcm_out[i],
            &mut b.adpcm_out[i],
            &mut b.scale[i],
            &mut b.total_distance[i],
        );
    }

    let mut best_coefficient = 0;

    let mut min = f64::MAX;

    for i in 0..8 {
        if b.total_distance[i] < min {
            min = b.total_distance[i];
            best_coefficient = i;
        }
    }

    for s in 0..sample_count {
        pcm_in_out[s + 2] = b.pcm_out[best_coefficient][s + 2] as i16;
    }

    adpcm_out[0] = combine_nibbles(best_coefficient as i32, b.scale[best_coefficient]);

    for s in sample_count..14 {
        b.adpcm_out[best_coefficient][s] = 0;
    }

    for i in 0..7 {
        adpcm_out[i + 1] = combine_nibbles(
            b.adpcm_out[best_coefficient][i * 2],
            b.adpcm_out[best_coefficient][i * 2 + 1],
        );
    }
}

fn dsp_encode_coefficient(
    pcm_in: &[i16],
    sample_count: usize,
    coefficients: &[i16],
    pcm_out: &mut [i32],
    adpcm_out: &mut [i32],
    scale_power: &mut i32,
    total_distance: &mut f64,
) {
    let mut max_overflow: i32;
    let mut max_distance: i32 = 0;

    pcm_out[0] = pcm_in[0] as i32;
    pcm_out[1] = pcm_in[1] as i32;

    // Encode the frame with a scale of 1
    for s in 0..sample_count {
        let input_sample: i32 = pcm_in[s + 2] as i32;
        let predicted_sample: i32 = (pcm_in[s] as i32 * coefficients[1] as i32
            + pcm_in[s + 1] as i32 * coefficients[0] as i32)
            / 2048;
        let distance: i32 = input_sample - predicted_sample;

        let distance: i16 = clamp_16(distance);

        if distance.abs() as i32 > max_distance.abs() {
            max_distance = distance as i32;
        }
    }

    // Use the maximum distance of the encoded frame to find a scale that will fit the current frame.
    *scale_power = 0;
    while *scale_power <= 12 && (max_distance > 7 || max_distance < -8) {
        max_distance /= 2;
        *scale_power += 1;
    }

    *scale_power = if *scale_power <= 1 { -1 } else { *scale_power - 2 };

    loop {
        *scale_power += 1;
        let scale: i32 = (1 << *scale_power) * 2048;
        *total_distance = 0.0;
        max_overflow = 0;

        for s in 0..sample_count {
            let input_sample: i32 = pcm_in[s + 2] as i32 * 2048;
            let predicted_sample = pcm_out[s] * coefficients[1] as i32
                + pcm_out[s + 1] as i32 * coefficients[0] as i32;
            let distance = input_sample - predicted_sample;

            let unclamped_adpcm_sample = if distance > 0 {
                let sample_tmp_f32: f32 = distance as f32 / scale as f32;
                let sample_tmp_f64 = sample_tmp_f32 as f64 + 0.4999999f64;

                sample_tmp_f64 as i32
            } else {
                let sample_tmp_f32: f32 = distance as f32 / scale as f32;
                let sample_tmp_f64 = sample_tmp_f32 as f64 - 0.4999999f64;

                sample_tmp_f64 as i32
            };

            let adpcm_sample = clamp_4(unclamped_adpcm_sample);

            if adpcm_sample as i32 != unclamped_adpcm_sample {
                let overflow = (unclamped_adpcm_sample - adpcm_sample as i32).abs();

                if overflow > max_overflow {
                    max_overflow = overflow;
                }
            }

            adpcm_out[s] = adpcm_sample as i32;

            // Decode sample to use as history
            let decoded_distance: i32 = adpcm_sample as i32 * scale;
            let corrected_sample = predicted_sample + decoded_distance as i32;
            let scaled_sample = (corrected_sample + 1024) >> 11;

            // Clamp and store
            pcm_out[s + 2] = clamp_16(scaled_sample) as i32;

            // Accumulate distance
            let actual_distance: f64 = pcm_in[s + 2] as f64 - pcm_out[s + 2] as f64;
            *total_distance += actual_distance * actual_distance;
        }

        let mut x = max_overflow;
        while x > 256 {
            *scale_power += 1;

            if *scale_power >= 12 {
                *scale_power = 11;
            }

            x >>= 1;
        }

        if *scale_power < 12 && max_overflow > 1 {
            continue;
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        coefficients::Coefficients, decode::decode_gc_adpcm, encode::encode_gc_adpcm,
        idsp::read_idsp_bytes,
    };
    use wav::{BitDepth, Header};

    #[test]
    fn test_encode_roundtrip() {
        let idsp_bytes = include_bytes!("../test_files/13.idsp");
        let idsp_file = read_idsp_bytes(idsp_bytes).unwrap();

        assert_eq!(idsp_file.channels.len(), 1);

        println!("encoded length before: {}", idsp_file.channels[0].audio.len());

        let decoded: Vec<i16> = decode_gc_adpcm(
            &idsp_file.channels[0].audio,
            &idsp_file.channels[0].metadata.coefficients,
        );

        let mut raw_pcm = Vec::new();
        for sample in decoded.iter() {
            raw_pcm.extend_from_slice(&sample.to_le_bytes());
        }

        let orig_coefs = &idsp_file.channels[0].metadata.coefficients;
        let encoded = encode_gc_adpcm(&decoded, orig_coefs);

        println!("encoded length after: {}", encoded.len());

        assert_eq!(idsp_file.channels[0].audio.len(), encoded.len());

        // for (original, new) in idsp_file.audio_data[0].iter().zip(encoded.iter()) {
        //     println!("{}, {}{}", original, new, if original != new { "!!!" } else { "" });
        // }

        let decoded_again: Vec<i16> =
            decode_gc_adpcm(&encoded, &idsp_file.channels[0].metadata.coefficients);

        let header =
            Header::new(1, idsp_file.channels.len() as u16, idsp_file.sample_rate as u32, 16);

        let mut output_file = std::fs::File::create("roundtrip.wav").unwrap();
        wav::write(header, BitDepth::Sixteen(decoded_again), &mut output_file).unwrap();
    }

    #[test]
    fn test_coefficient_proximity() {
        let idsp_bytes = include_bytes!("../test_files/13.idsp");
        let idsp_file = read_idsp_bytes(idsp_bytes).unwrap();

        assert_eq!(idsp_file.channels.len(), 1);

        println!("encoded length before: {}", idsp_file.channels[0].audio.len());

        let decoded: Vec<i16> = decode_gc_adpcm(
            &idsp_file.channels[0].audio,
            &idsp_file.channels[0].metadata.coefficients,
        );

        let mut raw_pcm = Vec::new();
        for sample in decoded.iter() {
            raw_pcm.extend_from_slice(&sample.to_le_bytes());
        }

        std::fs::write("raw_pcm.bin", &raw_pcm).unwrap();

        let coefs = Coefficients::from(&decoded[..]);
        let orig_coefs = &idsp_file.channels[0].metadata.coefficients;

        for (orig, new) in coefs.iter().zip(orig_coefs.iter()) {
            if ((orig - new) as f64).abs() > (orig.abs() as f64 * 0.01) {
                println!("orig: {:?}", orig_coefs);
                println!("calc: {:?}", *coefs);
                println!(
                    "{} - {} = {} > {}",
                    orig,
                    new,
                    (orig - new).abs(),
                    orig.abs() as f64 * 0.01
                );
                panic!("original and calculated coefficients differ more than 1%");
            }
        }
    }
}
