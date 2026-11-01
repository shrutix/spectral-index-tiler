//! A small JSON description of a tiling run, written without any JSON library.

use crate::colormap::Stretch;
use crate::index::Calibration;
use crate::stats::Stats;

/// Everything a viewer needs to interpret a folder of tiles.
#[derive(Debug, Clone)]
pub struct Manifest<'a> {
    /// `"ndvi"` or `"ndwi"`.
    pub index: &'a str,
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub tile_cols: u32,
    pub tile_rows: u32,
    /// Name of the color ramp used.
    pub ramp: &'a str,
    pub stretch: Stretch,
    pub calibration: Calibration,
    pub stats: Stats,
}

impl Manifest<'_> {
    pub fn to_json(&self) -> String {
        let nodata = self
            .calibration
            .nodata
            .map_or_else(|| "null".to_string(), |v| v.to_string());

        format!(
            "{{\n\
             \x20 \"index\": \"{index}\",\n\
             \x20 \"image\": {{ \"width\": {width}, \"height\": {height} }},\n\
             \x20 \"tiles\": {{ \"size\": {tile_size}, \"columns\": {cols}, \"rows\": {rows}, \
             \"pattern\": \"tile_r{{row:03}}_c{{col:03}}.png\" }},\n\
             \x20 \"color\": {{ \"ramp\": \"{ramp}\", \"min\": {smin}, \"max\": {smax} }},\n\
             \x20 \"calibration\": {{ \"offset\": {offset}, \"nodata\": {nodata} }},\n\
             \x20 \"stats\": {{ \"valid_pixels\": {valid}, \"invalid_pixels\": {invalid}, \
             \"min\": {min}, \"max\": {max}, \"mean\": {mean} }}\n\
             }}\n",
            index = escape(self.index),
            width = self.width,
            height = self.height,
            tile_size = self.tile_size,
            cols = self.tile_cols,
            rows = self.tile_rows,
            ramp = escape(self.ramp),
            smin = number_f32(self.stretch.min()),
            smax = number_f32(self.stretch.max()),
            offset = number_f32(self.calibration.offset),
            nodata = nodata,
            valid = self.stats.valid,
            invalid = self.stats.invalid,
            min = optional_f32(self.stats.min),
            max = optional_f32(self.stats.max),
            mean = optional_f64(self.stats.mean),
        )
    }
}

/// Escape a string for use inside a JSON string literal.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// JSON has no NaN or infinity, so those become `null`.
fn number_f32(value: f32) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".to_string()
    }
}

fn number_f64(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".to_string()
    }
}

fn optional_f32(value: Option<f32>) -> String {
    value.map_or_else(|| "null".to_string(), number_f32)
}

fn optional_f64(value: Option<f64>) -> String {
    value.map_or_else(|| "null".to_string(), number_f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::summarize;

    fn sample<'a>(stats: Stats, nodata: Option<u16>) -> Manifest<'a> {
        Manifest {
            index: "ndvi",
            width: 600,
            height: 300,
            tile_size: 256,
            tile_cols: 3,
            tile_rows: 2,
            ramp: "vegetation",
            stretch: Stretch::ndvi(),
            calibration: Calibration {
                offset: -1000.0,
                nodata,
            },
            stats,
        }
    }

    #[test]
    fn contains_every_field() {
        let json = sample(summarize(&[0.0, 0.5, f32::NAN]), Some(0)).to_json();
        for expected in [
            "\"index\": \"ndvi\"",
            "\"width\": 600",
            "\"height\": 300",
            "\"size\": 256",
            "\"columns\": 3",
            "\"rows\": 2",
            "\"pattern\": \"tile_r{row:03}_c{col:03}.png\"",
            "\"ramp\": \"vegetation\"",
            "\"min\": -0.2",
            "\"max\": 0.9",
            "\"offset\": -1000",
            "\"nodata\": 0",
            "\"valid_pixels\": 2",
            "\"invalid_pixels\": 1",
            "\"mean\": 0.25",
        ] {
            assert!(json.contains(expected), "missing {expected:?} in\n{json}");
        }
    }

    #[test]
    fn missing_values_become_null_never_nan() {
        let json = sample(summarize(&[f32::NAN]), None).to_json();
        assert!(json.contains("\"nodata\": null"));
        assert!(json.contains("\"min\": null, \"max\": null, \"mean\": null"));
        assert!(!json.to_lowercase().contains("nan"));
        assert!(!json.to_lowercase().contains("inf"));
    }

    #[test]
    fn braces_are_balanced_and_it_is_one_object() {
        let json = sample(summarize(&[0.1]), Some(0)).to_json();
        assert_eq!(json.matches('{').count(), json.matches('}').count());
        assert!(json.trim_start().starts_with('{') && json.trim_end().ends_with('}'));
    }

    #[test]
    fn escaping_handles_quotes_backslashes_and_controls() {
        assert_eq!(escape("plain"), "plain");
        assert_eq!(escape("a\"b"), "a\\\"b");
        assert_eq!(escape("a\\b"), "a\\\\b");
        assert_eq!(escape("a\nb\tc"), "a\\nb\\tc");
        assert_eq!(escape("\u{1}"), "\\u0001");
        assert_eq!(escape("café"), "café");
    }

    #[test]
    fn a_hostile_name_cannot_break_out_of_its_string() {
        let mut manifest = sample(summarize(&[0.1]), None);
        manifest.ramp = "x\", \"injected\": \"y";
        let json = manifest.to_json();
        assert!(!json.contains("\"injected\":"));
        assert!(json.contains("x\\\", \\\"injected\\\": \\\"y"));
    }
}
