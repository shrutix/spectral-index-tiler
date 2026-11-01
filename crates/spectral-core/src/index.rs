//! Normalized-difference indices over raw band values.
//!
//! A normalized difference is `(first - second) / (first + second)`:
//!
//! * NDVI (vegetation): first = near-infrared, second = red
//! * NDWI (open water, McFeeters): first = green, second = near-infrared
//!
//! Two properties are worth knowing, and the tests check both:
//!
//! * A **scale factor cancels** out of the ratio, so it never matters whether
//!   values are raw digital numbers or reflectance.
//! * An **offset does not cancel.** Sentinel-2 scenes processed after January
//!   2022 store reflectance as `(DN - 1000) / 10000`, so the -1000 must be
//!   applied before the ratio or the index is biased.

use crate::CoreError;
use std::thread;

/// How raw band values are prepared before the index is computed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calibration {
    /// Added to every raw value first. Use `-1000.0` for Sentinel-2 L2A scenes
    /// processed after January 2022 and `0.0` for older ones.
    pub offset: f32,
    /// A raw value that means "no data" (Sentinel-2 uses `0`). A pixel where
    /// either band holds this value becomes NaN.
    pub nodata: Option<u16>,
}

impl Default for Calibration {
    fn default() -> Self {
        Calibration {
            offset: 0.0,
            nodata: None,
        }
    }
}

/// Normalized difference for one pixel.
///
/// Returns NaN when the pixel is invalid: no-data in either band, or a
/// non-positive sum after the offset (the ratio is meaningless there).
/// Otherwise the result is clamped to `[-1, 1]`, since an offset can push one
/// band slightly negative and the ratio just beyond that range.
#[inline]
pub fn nd_pixel(first: u16, second: u16, cal: Calibration) -> f32 {
    if let Some(nodata) = cal.nodata {
        if first == nodata || second == nodata {
            return f32::NAN;
        }
    }
    let a = f32::from(first) + cal.offset;
    let b = f32::from(second) + cal.offset;
    let sum = a + b;
    if !sum.is_finite() || sum <= 0.0 {
        return f32::NAN;
    }
    ((a - b) / sum).clamp(-1.0, 1.0)
}

/// Fill `out` with the index for each pixel. All three slices must be the
/// same length (checked by the public wrappers).
fn nd_into(first: &[u16], second: &[u16], cal: Calibration, out: &mut [f32]) {
    debug_assert!(first.len() == second.len() && first.len() == out.len());
    for ((o, &a), &b) in out.iter_mut().zip(first).zip(second) {
        *o = nd_pixel(a, b, cal);
    }
}

fn check_lengths(first: &[u16], second: &[u16]) -> Result<(), CoreError> {
    if first.len() == second.len() {
        Ok(())
    } else {
        Err(CoreError::LengthMismatch {
            first: first.len(),
            second: second.len(),
        })
    }
}

/// Compute the index on the current thread.
pub fn normalized_difference(
    first: &[u16],
    second: &[u16],
    cal: Calibration,
) -> Result<Vec<f32>, CoreError> {
    check_lengths(first, second)?;
    let mut out = vec![0.0f32; first.len()];
    nd_into(first, second, cal, &mut out);
    Ok(out)
}

/// A sensible default number of worker threads for this machine.
pub fn default_threads() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Compute the index across `threads` threads.
///
/// The image is split into contiguous chunks, one per thread, and each thread
/// writes only its own part of the output. The result is identical to
/// [`normalized_difference`] bit for bit. Swapping this for `rayon`'s
/// `par_chunks_mut` is a good exercise once the baseline works.
pub fn normalized_difference_parallel(
    first: &[u16],
    second: &[u16],
    cal: Calibration,
    threads: usize,
) -> Result<Vec<f32>, CoreError> {
    check_lengths(first, second)?;
    let len = first.len();
    if len == 0 {
        return Ok(Vec::new());
    }

    let threads = threads.clamp(1, len);
    let chunk = len.div_ceil(threads);
    let mut out = vec![0.0f32; len];

    thread::scope(|scope| {
        for ((out_chunk, a), b) in out
            .chunks_mut(chunk)
            .zip(first.chunks(chunk))
            .zip(second.chunks(chunk))
        {
            scope.spawn(move || nd_into(a, b, cal, out_chunk));
        }
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Calibration = Calibration {
        offset: 0.0,
        nodata: None,
    };

    fn sentinel2() -> Calibration {
        Calibration {
            offset: -1000.0,
            nodata: Some(0),
        }
    }

    fn close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    // Expected values below were computed independently with NumPy (float32).

    #[test]
    fn matches_numpy_reference_values() {
        close(nd_pixel(3000, 1000, NONE), 0.5);
        close(nd_pixel(1000, 1000, NONE), 0.0);
        close(
            nd_pixel(
                2000,
                1500,
                Calibration {
                    offset: -1000.0,
                    nodata: None,
                },
            ),
            0.333_333_34,
        );
        close(
            nd_pixel(
                8000,
                2000,
                Calibration {
                    offset: -1000.0,
                    nodata: None,
                },
            ),
            0.75,
        );
    }

    #[test]
    fn swapping_the_bands_flips_the_sign() {
        for (a, b) in [(3000u16, 1000u16), (1200, 4800), (65535, 1)] {
            close(nd_pixel(a, b, NONE), -nd_pixel(b, a, NONE));
        }
    }

    #[test]
    fn a_scale_factor_cancels() {
        close(nd_pixel(3000, 1000, NONE), nd_pixel(9000, 3000, NONE));
        close(nd_pixel(100, 300, NONE), nd_pixel(10000, 30000, NONE));
    }

    #[test]
    fn an_offset_does_not_cancel() {
        let without = nd_pixel(2000, 1500, NONE);
        let with = nd_pixel(
            2000,
            1500,
            Calibration {
                offset: -1000.0,
                nodata: None,
            },
        );
        assert!((without - with).abs() > 0.1, "{without} vs {with}");
    }

    #[test]
    fn nodata_in_either_band_gives_nan() {
        let cal = sentinel2();
        assert!(nd_pixel(0, 3000, cal).is_nan());
        assert!(nd_pixel(3000, 0, cal).is_nan());
        assert!(nd_pixel(0, 0, cal).is_nan());
        assert!(!nd_pixel(3000, 2000, cal).is_nan());
    }

    #[test]
    fn zero_sum_gives_nan() {
        assert!(nd_pixel(0, 0, NONE).is_nan());
    }

    #[test]
    fn negative_sum_after_the_offset_gives_nan() {
        // 500 and 300 are both below the -1000 offset: both negative.
        assert!(nd_pixel(
            500,
            300,
            Calibration {
                offset: -1000.0,
                nodata: None
            }
        )
        .is_nan());
    }

    #[test]
    fn results_are_clamped_to_the_valid_range() {
        let cal = Calibration {
            offset: -1000.0,
            nodata: None,
        };
        // 500 and -100 after the offset: the raw ratio is 1.5.
        assert_eq!(nd_pixel(1500, 900, cal), 1.0);
        assert_eq!(nd_pixel(900, 1500, cal), -1.0);
    }

    #[test]
    fn a_band_at_exactly_zero_reflectance_gives_one() {
        let cal = Calibration {
            offset: -1000.0,
            nodata: None,
        };
        assert_eq!(nd_pixel(1200, 1000, cal), 1.0);
    }

    #[test]
    fn extreme_inputs_stay_in_range() {
        for (a, b) in [(65535u16, 65535u16), (65535, 1), (1, 65535), (1, 1)] {
            let v = nd_pixel(a, b, NONE);
            assert!((-1.0..=1.0).contains(&v), "{v}");
        }
    }

    #[test]
    fn length_mismatch_is_an_error() {
        let err = normalized_difference(&[1, 2, 3], &[1, 2], NONE).unwrap_err();
        assert_eq!(
            err,
            CoreError::LengthMismatch {
                first: 3,
                second: 2
            }
        );
        assert!(normalized_difference_parallel(&[1, 2, 3], &[1, 2], NONE, 4).is_err());
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(normalized_difference(&[], &[], NONE).unwrap().is_empty());
        assert!(normalized_difference_parallel(&[], &[], NONE, 8)
            .unwrap()
            .is_empty());
    }

    /// Deterministic pseudo-random values, many of them low enough to be
    /// invalid after a -1000 offset, plus some exact zeros.
    fn noisy(len: usize, seed: u64) -> Vec<u16> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((state >> 48) as u16) % 4000
            })
            .collect()
    }

    #[test]
    fn parallel_matches_sequential_bit_for_bit() {
        // An odd length makes the chunks uneven.
        let len = 100_003;
        let first = noisy(len, 1);
        let second = noisy(len, 2);
        let cal = sentinel2();

        let expected = normalized_difference(&first, &second, cal).unwrap();
        assert!(
            expected.iter().any(|v| v.is_nan()),
            "test data should include invalid pixels"
        );
        assert!(expected.iter().any(|v| !v.is_nan()));

        for threads in [1, 2, 3, 7, 16, 64] {
            let actual = normalized_difference_parallel(&first, &second, cal, threads).unwrap();
            assert_eq!(actual.len(), expected.len());
            for (i, (a, e)) in actual.iter().zip(&expected).enumerate() {
                assert_eq!(
                    a.to_bits(),
                    e.to_bits(),
                    "pixel {i} differs with {threads} threads"
                );
            }
        }
    }

    #[test]
    fn more_threads_than_pixels_is_fine() {
        let out = normalized_difference_parallel(&[3000, 1000], &[1000, 3000], NONE, 64).unwrap();
        close(out[0], 0.5);
        close(out[1], -0.5);
    }

    #[test]
    fn zero_threads_is_treated_as_one() {
        let out = normalized_difference_parallel(&[3000], &[1000], NONE, 0).unwrap();
        close(out[0], 0.5);
    }

    #[test]
    fn default_threads_is_at_least_one() {
        assert!(default_threads() >= 1);
    }
}
