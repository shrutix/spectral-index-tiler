//! End-to-end tests: write small synthetic GeoTIFFs, run the whole pipeline,
//! and read the PNG tiles back.

use spectral_core::colormap::Stretch;
use spectral_core::index::Calibration;
use spectral_tiler::pipeline::{run, IndexKind, Job};
use spectral_tiler::TilerError;
use std::fs::File;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tiff::encoder::{colortype, TiffEncoder};

fn write_band(path: &Path, width: u32, height: u32, data: &[u16]) {
    let mut file = File::create(path).unwrap();
    let mut encoder = TiffEncoder::new(&mut file).unwrap();
    encoder
        .write_image::<colortype::Gray16>(width, height, data)
        .unwrap();
}

/// Returns (width, height, RGBA bytes).
fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(File::open(path).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).unwrap();
    buffer.truncate(info.buffer_size());
    (info.width, info.height, buffer)
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * width + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

struct Fixture {
    _dir: TempDir,
    job: Job,
}

/// A 300x200 scene: red is constant, and NIR is high (vegetation, NDVI 0.5) on
/// the left and equal to red (bare ground, NDVI 0.0) from column 150 onward.
fn fixture(nodata: Option<u16>, zero_corner: bool) -> Fixture {
    let dir = TempDir::new().unwrap();
    let (width, height) = (300u32, 200u32);
    let mut red = vec![1000u16; (width * height) as usize];
    let mut nir: Vec<u16> = (0..width * height)
        .map(|i| if i % width < 150 { 3000 } else { 1000 })
        .collect();
    if zero_corner {
        red[0] = 0;
        nir[0] = 0;
    }
    write_band(&dir.path().join("red.tif"), width, height, &red);
    write_band(&dir.path().join("nir.tif"), width, height, &nir);

    let job = Job {
        first: dir.path().join("nir.tif"),
        second: dir.path().join("red.tif"),
        out_dir: dir.path().join("tiles"),
        kind: IndexKind::Ndvi,
        tile_size: 128,
        calibration: Calibration {
            offset: 0.0,
            nodata,
        },
        stretch: Stretch::ndvi(),
        threads: 2,
    };
    Fixture { _dir: dir, job }
}

#[test]
fn writes_every_tile_and_a_manifest() {
    let f = fixture(None, false);
    let report = run(&f.job).unwrap();

    // 300x200 in 128-pixel tiles: 3 columns by 2 rows.
    assert_eq!(report.tiles_written, 6);
    assert_eq!((report.width, report.height), (300, 200));
    for row in 0..2 {
        for col in 0..3 {
            let name = format!("tile_r{row:03}_c{col:03}.png");
            assert!(f.job.out_dir.join(&name).exists(), "missing {name}");
        }
    }
    let manifest = std::fs::read_to_string(f.job.out_dir.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"index\": \"ndvi\""));
    assert!(manifest.contains("\"columns\": 3"));
}

#[test]
fn edge_tiles_have_the_remaining_size() {
    let f = fixture(None, false);
    run(&f.job).unwrap();

    let (w, h, rgba) = read_png(&f.job.out_dir.join("tile_r000_c000.png"));
    assert_eq!((w, h), (128, 128));
    assert_eq!(rgba.len(), 128 * 128 * 4);

    // Bottom-right tile: 300 - 256 = 44 wide, 200 - 128 = 72 tall.
    let (w, h, _) = read_png(&f.job.out_dir.join("tile_r001_c002.png"));
    assert_eq!((w, h), (44, 72));
}

#[test]
fn vegetation_is_green_and_bare_ground_is_brown() {
    let f = fixture(None, false);
    run(&f.job).unwrap();

    // Tile column 0 is entirely vegetation; column 2 (x >= 256) is bare ground.
    let (w, _, veg) = read_png(&f.job.out_dir.join("tile_r000_c000.png"));
    let [vr, vg, _, va] = pixel(&veg, w, 10, 10);
    assert!(vg > vr, "vegetation should be green, got r={vr} g={vg}");
    assert_eq!(va, 255);

    let (w, _, soil) = read_png(&f.job.out_dir.join("tile_r000_c002.png"));
    let [sr, sg, _, sa] = pixel(&soil, w, 10, 10);
    assert!(sr > sg, "bare ground should be brown, got r={sr} g={sg}");
    assert_eq!(sa, 255);
}

#[test]
fn nodata_pixels_are_transparent_and_counted() {
    let f = fixture(Some(0), true);
    let report = run(&f.job).unwrap();

    assert_eq!(report.stats.invalid, 1);
    assert_eq!(report.stats.valid, 300 * 200 - 1);

    let (w, _, rgba) = read_png(&f.job.out_dir.join("tile_r000_c000.png"));
    assert_eq!(
        pixel(&rgba, w, 0, 0)[3],
        0,
        "the no-data pixel should be transparent"
    );
    assert_eq!(
        pixel(&rgba, w, 1, 0)[3],
        255,
        "its neighbour should be opaque"
    );
}

#[test]
fn statistics_match_the_synthetic_scene() {
    let f = fixture(None, false);
    let report = run(&f.job).unwrap();
    // Half the pixels are 0.5 and half are 0.0.
    assert!((report.stats.mean.unwrap() - 0.25).abs() < 1e-6);
    assert_eq!(report.stats.min, Some(0.0));
    assert_eq!(report.stats.max, Some(0.5));
}

#[test]
fn bands_of_different_sizes_are_rejected() {
    let dir = TempDir::new().unwrap();
    write_band(&dir.path().join("a.tif"), 10, 10, &[1000; 100]);
    write_band(&dir.path().join("b.tif"), 12, 10, &[1000; 120]);
    let job = Job {
        first: dir.path().join("a.tif"),
        second: dir.path().join("b.tif"),
        out_dir: dir.path().join("tiles"),
        kind: IndexKind::Ndvi,
        tile_size: 8,
        calibration: Calibration::default(),
        stretch: Stretch::ndvi(),
        threads: 1,
    };
    assert!(matches!(run(&job), Err(TilerError::SizeMismatch { .. })));
}

#[test]
fn a_missing_file_gives_a_clear_error() {
    let dir = TempDir::new().unwrap();
    let missing: PathBuf = dir.path().join("nope.tif");
    let job = Job {
        first: missing.clone(),
        second: missing,
        out_dir: dir.path().join("tiles"),
        kind: IndexKind::Ndvi,
        tile_size: 8,
        calibration: Calibration::default(),
        stretch: Stretch::ndvi(),
        threads: 1,
    };
    let error = run(&job).unwrap_err();
    assert!(matches!(error, TilerError::Open { .. }));
    assert!(error.to_string().contains("nope.tif"));
}

#[test]
fn water_index_runs_with_its_own_ramp() {
    let f = fixture(None, false);
    let mut job = f.job.clone();
    // NDWI = (green - NIR) / (green + NIR). Reuse the bands: "green" = red file.
    job.kind = IndexKind::Ndwi;
    job.stretch = Stretch::ndwi();
    job.first = f.job.second.clone();
    job.second = f.job.first.clone();
    let report = run(&job).unwrap();
    assert_eq!(report.tiles_written, 6);
    let manifest = std::fs::read_to_string(job.out_dir.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"index\": \"ndwi\""));
    assert!(manifest.contains("\"ramp\": \"water\""));
}
