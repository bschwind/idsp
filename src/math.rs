const SAMPLES_PER_FRAME: usize = 14;

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

struct Coefficients {
    inner: [i16; 16],
}

impl<'a> From<&'a [i16]> for Coefficients {
    fn from(source: &[i16]) -> Self {
        let frame_count = source.len().divide_by_round_up(SAMPLES_PER_FRAME);
        let mut pcm_hist = [0i16; SAMPLES_PER_FRAME * 2];
        let mut coefs = [0i16; 16];
        let mut vec1 = [0f64; 3];
        let mut vec2 = [0f64; 3];
        let mut buffer = [0f64; 3];
        let mut mtx = [[0f64; 3]; 3];
        let mut vec_idxs = [0usize, 3];
        let mut records = vec![vec![0f64; frame_count * 2]; 3];
        let mut record_count = 0;
        let mut vec_best = [[0f64; 3]; 8];

        for frame in source.windows(SAMPLES_PER_FRAME) {
            pcm_hist[SAMPLES_PER_FRAME..].copy_from_slice(frame);

            inner_product_merge(&mut vec1, &pcm_hist);
            if vec1[0].abs() > 10.0 {
                outer_product_merge(&mut mtx, &pcm_hist);
            }
        }

        Self { inner: [0; 16] }
    }
}

fn inner_product_merge(out: &mut [f64], pcm: &[i16]) {
    for i in 0..3 {
        out[i] = 0.0;
        for x in 0..SAMPLES_PER_FRAME {
            out[i] -= pcm[SAMPLES_PER_FRAME + x - i] as f64 * pcm[SAMPLES_PER_FRAME + x] as f64;
        }
    }
}

fn outer_product_merge(mtx: &mut [[f64; 3]], pcm: &[i16]) {
    for x in 0..3 {
        for y in 0..3 {
            mtx[x][y] = 0.0;
            for z in 0..SAMPLES_PER_FRAME {
                mtx[x][y] +=
                    pcm[SAMPLES_PER_FRAME + z - x] as f64 * pcm[SAMPLES_PER_FRAME + z - y] as f64;
            }
        }
    }
}
