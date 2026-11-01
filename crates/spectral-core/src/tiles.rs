//! Splitting an image into tiles and rendering each one.
//!
//! These are **pixel-grid tiles**: tile (0, 0) is the top-left block of the
//! source image. They are not web-map (XYZ) tiles, which would need the image
//! reprojected first.

use crate::colormap::{Ramp, Stretch};
use crate::CoreError;

/// One tile's position and size, in source pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileWindow {
    pub col: u32,
    pub row: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl TileWindow {
    /// The file name used for this tile, for example `tile_r002_c014.png`.
    pub fn file_name(&self) -> String {
        format!("tile_r{:03}_c{:03}.png", self.row, self.col)
    }
}

/// Split a `width` x `height` image into tiles of at most `tile_size` pixels.
///
/// Tiles are returned row by row, left to right. Tiles on the right and
/// bottom edges are smaller when the image size is not a multiple of
/// `tile_size`.
pub fn tile_grid(width: u32, height: u32, tile_size: u32) -> Result<Vec<TileWindow>, CoreError> {
    if tile_size == 0 {
        return Err(CoreError::InvalidTileSize);
    }
    if width == 0 || height == 0 {
        return Err(CoreError::EmptyImage);
    }

    let cols = width.div_ceil(tile_size);
    let rows = height.div_ceil(tile_size);
    let mut tiles = Vec::with_capacity(cols as usize * rows as usize);
    for row in 0..rows {
        for col in 0..cols {
            let x = col * tile_size;
            let y = row * tile_size;
            tiles.push(TileWindow {
                col,
                row,
                x,
                y,
                width: tile_size.min(width - x),
                height: tile_size.min(height - y),
            });
        }
    }
    Ok(tiles)
}

/// Render one tile of an index image as RGBA bytes (row-major, 4 per pixel).
///
/// `values` holds the whole image row by row, `image_width` values per row.
pub fn render_tile(
    values: &[f32],
    image_width: u32,
    window: &TileWindow,
    ramp: &Ramp,
    stretch: Stretch,
) -> Result<Vec<u8>, CoreError> {
    if image_width == 0 {
        return Err(CoreError::EmptyImage);
    }
    let image_width = image_width as usize;
    let image_height = values.len() / image_width;
    if image_height * image_width != values.len() {
        return Err(CoreError::InvalidWindow(format!(
            "{} values is not a whole number of {image_width}-pixel rows",
            values.len()
        )));
    }

    let (x, y) = (window.x as usize, window.y as usize);
    let (width, height) = (window.width as usize, window.height as usize);
    if width == 0 || height == 0 || x + width > image_width || y + height > image_height {
        return Err(CoreError::InvalidWindow(format!(
            "a {width}x{height} window at ({x}, {y}) does not fit in a {image_width}x{image_height} image"
        )));
    }

    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in y..y + height {
        let start = row * image_width + x;
        for &value in &values[start..start + width] {
            rgba.extend_from_slice(&ramp.apply(value, stretch));
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exact_fit_gives_equal_tiles() {
        let tiles = tile_grid(512, 512, 256).unwrap();
        assert_eq!(tiles.len(), 4);
        assert!(tiles.iter().all(|t| t.width == 256 && t.height == 256));
    }

    #[test]
    fn edge_tiles_are_smaller() {
        let tiles = tile_grid(600, 300, 256).unwrap();
        assert_eq!(tiles.len(), 3 * 2);
        let last = tiles.last().unwrap();
        assert_eq!((last.col, last.row), (2, 1));
        assert_eq!((last.x, last.y), (512, 256));
        assert_eq!((last.width, last.height), (88, 44));
    }

    #[test]
    fn tiles_come_out_row_by_row() {
        let tiles = tile_grid(600, 300, 256).unwrap();
        let order: Vec<_> = tiles.iter().map(|t| (t.row, t.col)).collect();
        assert_eq!(order, vec![(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]);
    }

    #[test]
    fn every_pixel_is_in_exactly_one_tile() {
        let (width, height) = (37u32, 23u32);
        let mut count = vec![0u32; (width * height) as usize];
        for tile in tile_grid(width, height, 8).unwrap() {
            for y in tile.y..tile.y + tile.height {
                for x in tile.x..tile.x + tile.width {
                    count[(y * width + x) as usize] += 1;
                }
            }
        }
        assert!(count.iter().all(|&c| c == 1));
    }

    #[test]
    fn a_tile_larger_than_the_image_gives_one_tile() {
        let tiles = tile_grid(100, 50, 256).unwrap();
        assert_eq!(tiles.len(), 1);
        assert_eq!((tiles[0].width, tiles[0].height), (100, 50));
    }

    #[test]
    fn a_one_pixel_image_works() {
        let tiles = tile_grid(1, 1, 256).unwrap();
        assert_eq!((tiles[0].width, tiles[0].height), (1, 1));
    }

    #[test]
    fn bad_sizes_are_rejected() {
        assert_eq!(tile_grid(10, 10, 0), Err(CoreError::InvalidTileSize));
        assert_eq!(tile_grid(0, 10, 4), Err(CoreError::EmptyImage));
        assert_eq!(tile_grid(10, 0, 4), Err(CoreError::EmptyImage));
    }

    #[test]
    fn file_names_are_zero_padded_and_sort_naturally() {
        let tiles = tile_grid(300, 300, 100).unwrap();
        assert_eq!(tiles[0].file_name(), "tile_r000_c000.png");
        assert_eq!(tiles[5].file_name(), "tile_r001_c002.png");
        let big = TileWindow {
            col: 14,
            row: 2,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        assert_eq!(big.file_name(), "tile_r002_c014.png");
    }

    fn gradient(width: usize, height: usize) -> Vec<f32> {
        // Each pixel holds its own flat index, so positions are easy to check.
        (0..width * height).map(|i| i as f32).collect()
    }

    fn grayscale() -> (Ramp, Stretch) {
        (
            Ramp::new(vec![(0.0, [0, 0, 0]), (1.0, [255, 255, 255])]).unwrap(),
            Stretch::new(0.0, 255.0).unwrap(),
        )
    }

    #[test]
    fn a_rendered_tile_has_four_bytes_per_pixel() {
        let (ramp, stretch) = grayscale();
        let values = gradient(10, 6);
        let window = TileWindow {
            col: 0,
            row: 0,
            x: 2,
            y: 1,
            width: 4,
            height: 3,
        };
        let rgba = render_tile(&values, 10, &window, &ramp, stretch).unwrap();
        assert_eq!(rgba.len(), 4 * 3 * 4);
    }

    #[test]
    fn a_rendered_tile_reads_the_right_pixels() {
        let (ramp, stretch) = grayscale();
        let values = gradient(10, 6);
        // Top-left pixel of this window is at x=2, y=1: flat index 12.
        let window = TileWindow {
            col: 0,
            row: 0,
            x: 2,
            y: 1,
            width: 4,
            height: 3,
        };
        let rgba = render_tile(&values, 10, &window, &ramp, stretch).unwrap();
        assert_eq!(&rgba[0..4], &ramp.apply(12.0, stretch));
        // Second row of the window starts at x=2, y=2: flat index 22.
        let second_row = 4 * 4;
        assert_eq!(
            &rgba[second_row..second_row + 4],
            &ramp.apply(22.0, stretch)
        );
        // Last pixel is x=5, y=3: flat index 35.
        let last = rgba.len() - 4;
        assert_eq!(&rgba[last..], &ramp.apply(35.0, stretch));
    }

    #[test]
    fn invalid_pixels_are_transparent_in_a_tile() {
        let (ramp, stretch) = grayscale();
        let mut values = gradient(4, 4);
        values[5] = f32::NAN; // x=1, y=1
        let window = TileWindow {
            col: 0,
            row: 0,
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        };
        let rgba = render_tile(&values, 4, &window, &ramp, stretch).unwrap();
        let alpha = |x: usize, y: usize| rgba[(y * 4 + x) * 4 + 3];
        assert_eq!(alpha(1, 1), 0);
        assert_eq!(alpha(0, 0), 255);
        assert_eq!(alpha(2, 1), 255);
    }

    #[test]
    fn a_window_outside_the_image_is_rejected() {
        let (ramp, stretch) = grayscale();
        let values = gradient(10, 6);
        let too_wide = TileWindow {
            col: 0,
            row: 0,
            x: 8,
            y: 0,
            width: 4,
            height: 2,
        };
        let too_tall = TileWindow {
            col: 0,
            row: 0,
            x: 0,
            y: 5,
            width: 2,
            height: 2,
        };
        let empty = TileWindow {
            col: 0,
            row: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 2,
        };
        for window in [too_wide, too_tall, empty] {
            assert!(matches!(
                render_tile(&values, 10, &window, &ramp, stretch),
                Err(CoreError::InvalidWindow(_))
            ));
        }
    }

    #[test]
    fn a_ragged_value_buffer_is_rejected() {
        let (ramp, stretch) = grayscale();
        let window = TileWindow {
            col: 0,
            row: 0,
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };
        assert!(render_tile(&[0.0; 7], 3, &window, &ramp, stretch).is_err());
        assert_eq!(
            render_tile(&[0.0; 6], 0, &window, &ramp, stretch),
            Err(CoreError::EmptyImage)
        );
    }

    #[test]
    fn grid_and_render_work_together_on_an_awkward_size() {
        let (ramp, stretch) = grayscale();
        let (width, height) = (37u32, 23u32);
        let values = gradient(width as usize, height as usize);
        let mut pixels = 0;
        for tile in tile_grid(width, height, 8).unwrap() {
            let rgba = render_tile(&values, width, &tile, &ramp, stretch).unwrap();
            assert_eq!(rgba.len(), (tile.width * tile.height * 4) as usize);
            pixels += rgba.len() / 4;
        }
        assert_eq!(pixels, (width * height) as usize);
    }
}
