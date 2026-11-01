//! Summary statistics that ignore invalid pixels.

/// Statistics over an index image. NaN and infinite values count as invalid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub valid: usize,
    pub invalid: usize,
    /// `None` when there are no valid pixels.
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub mean: Option<f64>,
}

/// Summarize an index image.
pub fn summarize(values: &[f32]) -> Stats {
    let mut valid = 0usize;
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut sum = 0.0f64;

    for &value in values {
        if value.is_finite() {
            valid += 1;
            min = min.min(value);
            max = max.max(value);
            sum += f64::from(value);
        }
    }

    let any = valid > 0;
    Stats {
        valid,
        invalid: values.len() - valid,
        min: any.then_some(min),
        max: any.then_some(max),
        mean: any.then(|| sum / valid as f64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_valid_values() {
        let stats = summarize(&[0.0, 0.5, 1.0, -0.5]);
        assert_eq!(stats.valid, 4);
        assert_eq!(stats.invalid, 0);
        assert_eq!(stats.min, Some(-0.5));
        assert_eq!(stats.max, Some(1.0));
        assert!((stats.mean.unwrap() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn ignores_nan_and_infinity() {
        let stats = summarize(&[f32::NAN, 0.2, f32::INFINITY, 0.4, f32::NEG_INFINITY]);
        assert_eq!((stats.valid, stats.invalid), (2, 3));
        assert_eq!(stats.min, Some(0.2));
        assert_eq!(stats.max, Some(0.4));
        assert!((stats.mean.unwrap() - 0.3).abs() < 1e-6);
    }

    #[test]
    fn all_invalid_has_no_range_or_mean() {
        let stats = summarize(&[f32::NAN, f32::NAN]);
        assert_eq!((stats.valid, stats.invalid), (0, 2));
        assert_eq!((stats.min, stats.max, stats.mean), (None, None, None));
    }

    #[test]
    fn empty_input_is_all_none() {
        let stats = summarize(&[]);
        assert_eq!((stats.valid, stats.invalid), (0, 0));
        assert_eq!(stats.min, None);
    }

    #[test]
    fn the_mean_stays_accurate_over_many_pixels() {
        // f32 accumulation would drift here; the f64 accumulator does not.
        let values = vec![0.1f32; 5_000_000];
        let mean = summarize(&values).mean.unwrap();
        assert!((mean - f64::from(0.1f32)).abs() < 1e-9, "{mean}");
    }
}
