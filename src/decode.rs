use crate::{
    byte_count_to_sample_count, clamp_16, high_nibble, high_nibble_signed, low_nibble,
    low_nibble_signed, CodecParameters, DivideByRoundUp, SAMPLES_PER_FRAME,
};

pub fn decode_gc_adpcm(adpcm: &[u8], coefficients: &[i16]) -> Vec<i16> {
    let config = CodecParameters {
        sample_count: byte_count_to_sample_count(adpcm.len()),
        history_1: 0,
        history_2: 0,
    };

    let mut pcm = vec![0; config.sample_count];

    if config.sample_count == 0 {
        return pcm;
    }

    let frame_count = config.sample_count.divide_by_round_up(SAMPLES_PER_FRAME);
    let mut current_sample = 0;
    let mut out_index = 0;
    let mut in_index = 0;
    let mut hist_1 = config.history_1;
    let mut hist_2 = config.history_2;

    for _i in 0..frame_count {
        let predictor_scale: u8 = adpcm[in_index];
        in_index += 1;

        let scale: i32 = (1 << low_nibble(predictor_scale)) as i32 * 2048;
        let predictor: i32 = high_nibble(predictor_scale) as i32;
        let coef_1: i16 = coefficients[predictor as usize * 2];
        let coef_2: i16 = coefficients[predictor as usize * 2 + 1];

        let samples_to_read: i32 =
            SAMPLES_PER_FRAME.min(config.sample_count - current_sample) as i32;

        for s in 0..samples_to_read {
            let adpcm_sample: i32 = if s % 2 == 0 {
                high_nibble_signed(adpcm[in_index]) as i32
            } else {
                let sample = low_nibble_signed(adpcm[in_index]);
                in_index += 1;
                sample as i32
            };

            let distance: i32 = scale * adpcm_sample;
            // TODO(bschwind) - should these coefficients be casted here?
            let predicted_sample: i32 =
                coef_1 as i32 * hist_1 as i32 + coef_2 as i32 * hist_2 as i32;
            let corrected_sample: i32 = predicted_sample as i32 + distance;
            let scaled_sample: i32 = (corrected_sample + 1024) >> 11;

            let clamped_sample: i16 = clamp_16(scaled_sample);

            hist_2 = hist_1;
            hist_1 = clamped_sample;

            pcm[out_index] = clamped_sample;
            out_index += 1;
            current_sample += 1;
        }
    }

    pcm
}
