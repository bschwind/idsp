use crate::{math::DivideByRoundUp, SAMPLES_PER_FRAME};

#[derive(Debug)]
pub struct Coefficients {
    pub coefs: [i16; 16],
}

impl std::ops::Deref for Coefficients {
    type Target = [i16; 16];

    fn deref(&self) -> &Self::Target {
        &self.coefs
    }
}

impl<T: AsRef<[i16]>> From<T> for Coefficients {
    fn from(source: T) -> Self {
        let source = source.as_ref();
        let frame_count = source.len().divide_by_round_up(SAMPLES_PER_FRAME);
        let mut pcm_hist = [0i16; SAMPLES_PER_FRAME * 2];
        let mut coefs = [0i16; 16];
        let mut vec1 = [0f64; 3];
        let mut vec2 = [0f64; 3];
        let mut buffer = [0f64; 3];
        let mut mtx = [[0f64; 3]; 3];
        let mut vec_idxs = [0usize; 3];
        let mut records = vec![vec![0f64; 3]; frame_count * 2];
        let mut record_count = 0;
        let mut vec_best = [[0f64; 3]; 8];

        for frame in source.chunks(SAMPLES_PER_FRAME) {
            pcm_hist[SAMPLES_PER_FRAME..SAMPLES_PER_FRAME + frame.len()].copy_from_slice(frame);

            inner_product_merge(&mut vec1, &pcm_hist);
            if vec1[0].abs() > 10.0 {
                outer_product_merge(&mut mtx, &pcm_hist);
                if !analyze_ranges(&mut mtx, &mut vec_idxs, &mut buffer) {
                    bidirectional_filter(&mut mtx, &mut vec_idxs, &mut vec1);
                    if !quadratic_merge(&mut vec1) {
                        finish_record(&mut vec1, &mut records[record_count]);
                        record_count += 1;
                    }
                }
            }

            let (a, b) = pcm_hist.split_at_mut(SAMPLES_PER_FRAME);
            a.copy_from_slice(b);
        }

        vec1[0] = 1.0;
        vec1[1] = 0.0;
        vec1[2] = 0.0;

        for z in 0..record_count {
            matrix_filter(&mut records, z, &mut vec_best[0], &mut mtx);
            for y in 1..=2 {
                vec1[y] += vec_best[0][y];
            }
        }

        for y in 1..=2 {
            vec1[y] /= record_count as f64;
        }

        merge_finish_record(&mut vec1, &mut vec_best[0]);

        let mut exp = 1;
        let mut w = 0;
        while w < 3 {
            vec2[0] = 0.0;
            vec2[1] = -1.0;
            vec2[2] = 0.0;
            for i in 0..exp {
                for y in 0..=2 {
                    vec_best[exp + i][y] = (0.01 * vec2[y]) + vec_best[i][y];
                }
            }
            w += 1;
            exp = 1 << w;
            filter_records(&mut vec_best, exp, &mut records, record_count);
        }

        for z in 0..8 {
            let mut d = -vec_best[z][1] * 2048.0;
            if d > 0.0 {
                coefs[z * 2] = if d > std::i16::MAX as f64 { i16::MAX } else { d.round() as i16 };
            } else {
                coefs[z * 2] = if d < std::i16::MIN as f64 { i16::MIN } else { d.round() as i16 };
            }

            d = -vec_best[z][2] * 2048.0;
            if d > 0.0 {
                coefs[z * 2 + 1] =
                    if d > std::i16::MAX as f64 { i16::MAX } else { d.round() as i16 };
            } else {
                coefs[z * 2 + 1] =
                    if d < std::i16::MIN as f64 { i16::MIN } else { d.round() as i16 };
            };
        }

        Self { coefs }
    }
}

fn inner_product_merge(out: &mut [f64], pcm: &[i16]) {
    for i in 0..=2 {
        out[i] = 0.0;
        for x in 0..14 {
            out[i] -= pcm[14 + x - i] as f64 * pcm[14 + x] as f64;
        }
    }
}

fn outer_product_merge(mtx: &mut [[f64; 3]], pcm: &[i16]) {
    for x in 1..=2 {
        for y in 1..=2 {
            mtx[x][y] = 0.0;
            for z in 0..14 {
                mtx[x][y] += pcm[14 + z - x] as f64 * pcm[14 + z - y] as f64;
            }
        }
    }
}

fn analyze_ranges(mtx: &mut [[f64; 3]], vec_idxs: &mut [usize], recips: &mut [f64]) -> bool {
    let mut val;
    let mut tmp;
    let mut min;
    let mut max;

    // Get greatest distance from zero
    for x in 1..=2 {
        val = mtx[x][1].abs().max(mtx[x][2].abs());
        if val < f64::EPSILON {
            return true;
        }

        recips[x] = 1.0 / val;
    }

    let mut max_index = 0;
    for i in 1..=2 {
        for x in 1..i {
            tmp = mtx[x][i];
            for y in 1..x {
                tmp -= mtx[x][y] * mtx[y][i];
            }
            mtx[x][i] = tmp;
        }

        val = 0.0;
        for x in i..=2 {
            tmp = mtx[x][i];
            for y in 1..i {
                tmp -= mtx[x][y] * mtx[y][i];
            }

            mtx[x][i] = tmp;
            tmp = tmp.abs() * recips[x];
            if tmp >= val {
                val = tmp;
                max_index = x;
            }
        }

        if max_index != i {
            for y in 1..=2 {
                tmp = mtx[max_index][y];
                mtx[max_index][y] = mtx[i][y];
                mtx[i][y] = tmp;
            }
            recips[max_index] = recips[i];
        }
        vec_idxs[i] = max_index;

        if i != 2 {
            tmp = 1.0 / mtx[i][i];
            for x in (i + 1)..=2 {
                mtx[x][i] *= tmp;
            }
        }
    }

    min = 1.0e10;
    max = 0.0;

    for i in 1..=2 {
        tmp = mtx[i][i].abs();
        if tmp < min {
            min = tmp;
        }
        if tmp > max {
            max = tmp;
        }
    }

    min / max < 1.0e-10
}

fn bidirectional_filter(mtx: &mut [[f64; 3]], vec_idxs: &mut [usize], vec_out: &mut [f64]) {
    let mut tmp;
    let mut x = 0;
    for i in 1..=2 {
        let index = vec_idxs[i];
        tmp = vec_out[index];
        vec_out[index] = vec_out[i];
        if x != 0 {
            for y in x..=i - 1 {
                tmp -= vec_out[y] * mtx[i][y];
            }
        } else if tmp != 0.0 {
            x = i;
        }
        vec_out[i] = tmp;
    }

    for i in (1..=2).rev() {
        tmp = vec_out[i];
        for y in i + 1..=2 {
            tmp -= vec_out[y] * mtx[i][y];
        }
        vec_out[i] = tmp / mtx[i][i];
    }

    vec_out[0] = 1.0;
}

fn quadratic_merge(in_out: &mut [f64]) -> bool {
    let v2 = in_out[2];
    let tmp = 1.0 - (v2 * v2);

    if tmp == 0.0 {
        return true;
    }

    let v0 = (in_out[0] - (v2 * v2)) / tmp;
    let v1 = (in_out[1] - (in_out[1] * v2)) / tmp;

    in_out[0] = v0;
    in_out[1] = v1;

    v1.abs() > 1.0
}

fn finish_record(in_r: &mut [f64], out_r: &mut [f64]) {
    for z in 1..=2 {
        if in_r[z] >= 1.0 {
            in_r[z] = 0.9999999999;
        } else if in_r[z] <= -1.0 {
            in_r[z] = -0.9999999999;
        }
    }

    out_r[0] = 1.0;
    out_r[1] = (in_r[2] * in_r[1]) + in_r[1];
    out_r[2] = in_r[2];
}

fn matrix_filter(src: &mut [Vec<f64>], row: usize, dst: &mut [f64], mtx: &mut [[f64; 3]]) {
    mtx[2][0] = 1.0;
    for i in 1..=2 {
        mtx[2][i] = -src[row][i];
    }

    for i in (1..=2).rev() {
        let val = 1.0 - (mtx[i][i] * mtx[i][i]);
        for y in 1..=i {
            mtx[i - 1][y] = ((mtx[i][i] * mtx[i][y]) + mtx[i][y]) / val;
        }
    }

    dst[0] = 1.0;
    for i in 1..=2 {
        dst[i] = 0.0;
        for y in 1..=i {
            dst[i] += mtx[i][y] * dst[i - y];
        }
    }
}

fn merge_finish_record(src: &[f64], dst: &mut [f64]) {
    let mut tmp = [0f64; 3];
    let mut val = src[0];

    dst[0] = 1.0;
    for i in 1..=2 {
        let mut v2 = 0.0;
        for y in 1..i {
            v2 += dst[y] * src[i - y];
        }

        if val > 0.0 {
            dst[i] = -(v2 + src[i]) / val;
        } else {
            dst[i] = 0.0;
        }

        tmp[i] = dst[i];

        for y in 1..i {
            dst[i] += dst[i] * dst[i - y];
        }

        val *= 1.0 - (dst[i] * dst[i]);
    }

    finish_record(&mut tmp, dst);
}

fn contrast_vectors(source1: &[f64], source2: &[f64]) -> f64 {
    let val = (source2[2] * source2[1] + -source2[1]) / (1.0 - source2[2] * source2[2]);
    let val1 = (source1[0] * source1[0]) + (source1[1] * source1[1]) + (source1[2] * source1[2]);
    let val2 = (source1[0] * source1[1]) + (source1[1] * source1[2]);
    let val3 = source1[0] * source1[2];
    val1 + (2.0 * val * val2) + (2.0 * (-source2[1] * val + -source2[2]) * val3)
}

fn filter_records(
    vec_best: &mut [[f64; 3]],
    exp: usize,
    records: &mut [Vec<f64>],
    record_count: usize,
) {
    let mut buffer_list = [[0f64; 3]; 8];
    let mut mtx = [[0f64; 3]; 3];
    let mut buffer1 = [0usize; 8];
    let mut buffer2 = [0f64; 3];

    for _x in 0..2 {
        for y in 0..exp {
            buffer1[y] = 0;
            for i in 0..=2 {
                buffer_list[y][i] = 0.0;
            }
        }
        for z in 0..record_count {
            let mut index = 0;
            let mut value = 1.0e30;
            for i in 0..exp {
                let temp_val = contrast_vectors(&mut vec_best[i], &mut records[z]);
                if temp_val < value {
                    value = temp_val;
                    index = i;
                }
            }
            buffer1[index] += 1;
            matrix_filter(records, z, &mut buffer2, &mut mtx);
            for i in 0..=2 {
                buffer_list[index][i] += buffer2[i];
            }
        }

        for i in 0..exp {
            if buffer1[i] > 0 {
                for y in 0..=2 {
                    buffer_list[i][y] /= buffer1[i] as f64;
                }
            }
        }

        for i in 0..exp {
            merge_finish_record(&mut buffer_list[i], &mut vec_best[i]);
        }
    }
}
