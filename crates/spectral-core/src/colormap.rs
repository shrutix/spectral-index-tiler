//! Color ramps and value stretches.
//!
//! A [`Stretch`] maps an index value onto `0.0..=1.0`, and a [`Ramp`] maps that
//! position onto a color. Invalid (NaN) pixels become fully transparent.

use crate::CoreError;
use std::cmp::Ordering;

pub type Rgb = [u8; 3];
pub type Rgba = [u8; 4];

/// The color of an invalid pixel: fully transparent.
pub const TRANSPARENT: Rgba = [0, 0, 0, 0];

/// The index range that is spread across the whole color ramp. Values outside
/// it take the ramp's end colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stretch {
    min: f32,
    max: f32,
}

impl Stretch {
    pub fn new(min: f32, max: f32) -> Result<Self, CoreError> {
        if min.is_finite() && max.is_finite() && min < max {
            Ok(Stretch { min, max })
        } else {
            Err(CoreError::InvalidRamp(format!(
                "the stretch needs finite values with min < max (got {min} and {max})"
            )))
        }
    }

    /// A usual range for NDVI: water below zero, dense vegetation near 0.9.
    pub fn ndvi() -> Self {
        Stretch {
            min: -0.2,
            max: 0.9,
        }
    }

    /// A usual range for NDWI (McFeeters): open water is positive.
    pub fn ndwi() -> Self {
        Stretch {
            min: -0.5,
            max: 0.8,
        }
    }

    pub fn min(&self) -> f32 {
        self.min
    }

    pub fn max(&self) -> f32 {
        self.max
    }

    fn position(&self, value: f32) -> f32 {
        (value - self.min) / (self.max - self.min)
    }
}

/// A piecewise-linear color ramp over positions `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq)]
pub struct Ramp {
    stops: Vec<(f32, Rgb)>,
}

impl Ramp {
    /// Build a ramp from `(position, color)` stops. Positions must start at
    /// `0.0`, end at `1.0`, and strictly increase.
    pub fn new(stops: Vec<(f32, Rgb)>) -> Result<Self, CoreError> {
        let invalid = |message: &str| Err(CoreError::InvalidRamp(message.to_string()));

        if stops.len() < 2 {
            return invalid("a ramp needs at least two stops");
        }
        if stops[0].0 != 0.0 || stops[stops.len() - 1].0 != 1.0 {
            return invalid("stop positions must run from 0.0 to 1.0");
        }
        // Anything other than "strictly less" is rejected, which includes NaN.
        let increasing =
            |pair: &[(f32, Rgb)]| pair[0].0.partial_cmp(&pair[1].0) == Some(Ordering::Less);
        if !stops.windows(2).all(increasing) {
            return invalid("stop positions must strictly increase");
        }
        Ok(Ramp { stops })
    }

    /// Blue (water), brown (bare soil), yellow, then light to dark green.
    pub fn vegetation() -> Self {
        Ramp::new(vec![
            (0.00, [49, 70, 140]),
            (0.18, [166, 124, 82]),
            (0.36, [230, 215, 120]),
            (0.55, [160, 200, 90]),
            (0.73, [60, 150, 60]),
            (1.00, [0, 80, 30]),
        ])
        .expect("the built-in vegetation ramp is valid")
    }

    /// Tan (dry land), near-white, light blue, then deep blue (open water).
    pub fn water() -> Self {
        Ramp::new(vec![
            (0.00, [190, 160, 110]),
            (0.40, [245, 245, 235]),
            (0.60, [140, 190, 230]),
            (1.00, [8, 48, 107]),
        ])
        .expect("the built-in water ramp is valid")
    }

    /// The color at a position. Positions outside `0..=1` take the end colors,
    /// and NaN takes the first color.
    pub fn color_at(&self, position: f32) -> Rgb {
        if position.is_nan() {
            return self.stops[0].1;
        }
        let position = position.clamp(0.0, 1.0);
        for pair in self.stops.windows(2) {
            let (start, from) = pair[0];
            let (end, to) = pair[1];
            if position <= end {
                let f = (position - start) / (end - start);
                return [
                    lerp(from[0], to[0], f),
                    lerp(from[1], to[1], f),
                    lerp(from[2], to[2], f),
                ];
            }
        }
        self.stops[self.stops.len() - 1].1
    }

    /// The color of one index value, or [`TRANSPARENT`] if it is not finite.
    pub fn apply(&self, value: f32, stretch: Stretch) -> Rgba {
        if !value.is_finite() {
            return TRANSPARENT;
        }
        let [r, g, b] = self.color_at(stretch.position(value));
        [r, g, b, 255]
    }
}

fn lerp(from: u8, to: u8, f: f32) -> u8 {
    let (from, to) = (f32::from(from), f32::from(to));
    (from + (to - from) * f).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn black_to_white() -> Ramp {
        Ramp::new(vec![(0.0, [0, 0, 0]), (1.0, [255, 255, 255])]).unwrap()
    }

    #[test]
    fn end_positions_give_the_end_colors() {
        let ramp = Ramp::vegetation();
        assert_eq!(ramp.color_at(0.0), [49, 70, 140]);
        assert_eq!(ramp.color_at(1.0), [0, 80, 30]);
    }

    #[test]
    fn the_midpoint_is_interpolated() {
        assert_eq!(black_to_white().color_at(0.5), [128, 128, 128]);
        assert_eq!(black_to_white().color_at(0.25), [64, 64, 64]);
    }

    #[test]
    fn a_position_on_a_stop_gives_exactly_that_stop() {
        assert_eq!(Ramp::vegetation().color_at(0.18), [166, 124, 82]);
    }

    #[test]
    fn positions_outside_the_range_clamp() {
        let ramp = black_to_white();
        assert_eq!(ramp.color_at(-5.0), [0, 0, 0]);
        assert_eq!(ramp.color_at(7.0), [255, 255, 255]);
        assert_eq!(ramp.color_at(f32::INFINITY), [255, 255, 255]);
        assert_eq!(ramp.color_at(f32::NAN), [0, 0, 0]);
    }

    #[test]
    fn invalid_pixels_are_transparent() {
        let ramp = Ramp::vegetation();
        let stretch = Stretch::ndvi();
        assert_eq!(ramp.apply(f32::NAN, stretch), TRANSPARENT);
        assert_eq!(ramp.apply(f32::INFINITY, stretch), TRANSPARENT);
        assert_eq!(ramp.apply(f32::NEG_INFINITY, stretch), TRANSPARENT);
    }

    #[test]
    fn valid_pixels_are_opaque() {
        let ramp = Ramp::vegetation();
        for value in [-1.0, -0.2, 0.0, 0.5, 0.9, 1.0] {
            assert_eq!(ramp.apply(value, Stretch::ndvi())[3], 255);
        }
    }

    #[test]
    fn the_stretch_maps_its_ends_to_the_ramp_ends() {
        let ramp = black_to_white();
        let stretch = Stretch::new(-0.2, 0.9).unwrap();
        assert_eq!(ramp.apply(-0.2, stretch), [0, 0, 0, 255]);
        assert_eq!(ramp.apply(0.9, stretch), [255, 255, 255, 255]);
        // Beyond the stretch the ends are held.
        assert_eq!(ramp.apply(-1.0, stretch), [0, 0, 0, 255]);
        assert_eq!(ramp.apply(1.0, stretch), [255, 255, 255, 255]);
    }

    #[test]
    fn the_stretch_midpoint_gives_the_ramp_midpoint() {
        let stretch = Stretch::new(-1.0, 1.0).unwrap();
        assert_eq!(black_to_white().apply(0.0, stretch), [128, 128, 128, 255]);
    }

    #[test]
    fn denser_vegetation_is_greener_than_bare_soil() {
        let ramp = Ramp::vegetation();
        let stretch = Stretch::ndvi();
        let [sr, sg, _, _] = ramp.apply(0.0, stretch);
        let [vr, vg, _, _] = ramp.apply(0.7, stretch);
        assert!(sr > sg, "soil should be brown (red above green)");
        assert!(vg > vr, "vegetation should be green (green above red)");
    }

    #[test]
    fn open_water_is_bluer_than_dry_land_in_the_water_ramp() {
        let ramp = Ramp::water();
        let stretch = Stretch::ndwi();
        let [_, _, land_b, _] = ramp.apply(-0.4, stretch);
        let [wr, _, wb, _] = ramp.apply(0.7, stretch);
        assert!(wb > wr);
        assert!(land_b < 255);
        let [lr, _, _, _] = ramp.apply(-0.4, stretch);
        assert!(lr > wr, "land should be warmer (more red) than water");
    }

    #[test]
    fn built_in_ramps_and_stretches_are_valid() {
        let _ = Ramp::vegetation();
        let _ = Ramp::water();
        for stretch in [Stretch::ndvi(), Stretch::ndwi()] {
            assert!(stretch.min() < stretch.max());
        }
    }

    #[test]
    fn invalid_ramps_are_rejected() {
        assert!(Ramp::new(vec![(0.0, [0, 0, 0])]).is_err());
        assert!(Ramp::new(vec![(0.1, [0, 0, 0]), (1.0, [1, 1, 1])]).is_err());
        assert!(Ramp::new(vec![(0.0, [0, 0, 0]), (0.9, [1, 1, 1])]).is_err());
        assert!(Ramp::new(vec![
            (0.0, [0, 0, 0]),
            (0.5, [1, 1, 1]),
            (0.5, [2, 2, 2]),
            (1.0, [3, 3, 3])
        ])
        .is_err());
        assert!(Ramp::new(vec![
            (0.0, [0, 0, 0]),
            (0.7, [1, 1, 1]),
            (0.3, [2, 2, 2]),
            (1.0, [3, 3, 3])
        ])
        .is_err());
        assert!(Ramp::new(vec![
            (0.0, [0, 0, 0]),
            (f32::NAN, [1, 1, 1]),
            (1.0, [3, 3, 3])
        ])
        .is_err());
    }

    #[test]
    fn invalid_stretches_are_rejected() {
        assert!(Stretch::new(0.5, 0.5).is_err());
        assert!(Stretch::new(1.0, 0.0).is_err());
        assert!(Stretch::new(f32::NAN, 1.0).is_err());
        assert!(Stretch::new(0.0, f32::INFINITY).is_err());
        assert!(Stretch::new(-0.2, 0.9).is_ok());
    }
}
