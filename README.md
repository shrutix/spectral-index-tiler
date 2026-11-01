# Spectral Index Tiler

A Rust command-line tool that turns two satellite image bands into colored **NDVI** (vegetation) or **NDWI** (open water) PNG tiles, with the index computed in parallel across threads.

The numerical core is dependency-free Rust, it is checked against independent NumPy reference values, and it is benchmarked against a NumPy implementation of the same calculation.

## Features

- **NDVI and NDWI** from two single-band GeoTIFFs (for example Sentinel-2 bands B04/B08 and B03/B08)
- **Correct handling of real-world data**: a configurable offset (Sentinel-2 scenes processed after January 2022 need `-1000`), a no-data value, and safe treatment of zero or negative sums
- **Multi-threaded** using only the standard library (`std::thread::scope`), with output identical to the single-threaded version bit for bit
- **Colored PNG tiles** with a transparent background for invalid pixels, plus a `manifest.json` describing the run
- **Dependency-free core crate** that compiles and tests with no network access
- **Tests**: index math checked against NumPy, tiling coverage, colormaps, JSON output, and end-to-end runs on synthetic GeoTIFFs

## How it works

```
spectral-tiler (CLI crate)            spectral-core (no dependencies)
────────────────────────────          ───────────────────────────────
raster.rs   read GeoTIFF bands   ──►  index.rs     normalized difference, parallel
pipeline.rs orchestrate the job  ──►  stats.rs     summary statistics
output.rs   write PNG tiles      ──►  colormap.rs  ramps and stretches
main.rs     command line         ──►  tiles.rs     tile grid and rendering
                                      manifest.rs  JSON description
```

All of the numerical and image logic lives in `spectral-core`, so it can be tested and benchmarked without any file I/O. The CLI crate only handles files and arguments.

### The two things people get wrong

A normalized difference is `(first - second) / (first + second)`.

- **A scale factor cancels.** Whether you use raw digital numbers or reflectance, the ratio is the same, so there is no need to divide by 10,000. A test checks this.
- **An offset does not cancel.** Sentinel-2 scenes processed after January 2022 store reflectance as `(DN - 1000) / 10000`. Skipping the `-1000` biases every pixel, and the error is large over dark surfaces. Pass `--offset -1000` for those scenes, and check your scene's metadata for its value.

### Pixels that are not valid

A pixel becomes transparent (NaN internally) when either band holds the no-data value, or when the two bands sum to zero or less after the offset. Valid results are clamped to the range -1 to 1, since an offset can push a band slightly negative and the ratio just past that range.

## Getting started

Install Rust with [rustup](https://rustup.rs), then:

```bash
git clone <https://github.com/shrutix/spectral-index-tiler.git>
cd spectral-tiler
cargo test                # builds both crates and runs every test
cargo build --release
```

### Get some imagery

[Sentinel-2 Cloud-Optimized GeoTIFFs](https://registry.opendata.aws/sentinel-2-l2a-cogs/) are free on the AWS Open Data registry. Find a scene through a STAC catalog such as Element 84's Earth Search, note the URLs of the red, green, and near-infrared band files, then clip a window so you do not download a whole 240 MB band:

```bash
pip install rasterio
python scripts/clip_band.py <red-band-url>  red.tif   --x 4000 --y 4000 --size 2048
python scripts/clip_band.py <nir-band-url>  nir.tif   --x 4000 --y 4000 --size 2048
python scripts/clip_band.py <green-band-url> green.tif --x 4000 --y 4000 --size 2048
```

Use the same window for every band, and use bands with the same resolution (red, green, and NIR are all 10 m on Sentinel-2). Do not commit imagery to the repository.

### Run it

```bash
cargo run --release -p spectral-tiler -- ndvi \
    --red red.tif --nir nir.tif --out tiles/ndvi \
    --offset -1000 --nodata 0

cargo run --release -p spectral-tiler -- ndwi \
    --green green.tif --nir nir.tif --out tiles/ndwi \
    --offset -1000 --nodata 0
```

| Option | Meaning | Default |
| --- | --- | --- |
| `--out` | folder for tiles and `manifest.json` | required |
| `--tile-size` | tile size in pixels | 256 |
| `--offset` | added to raw values first | 0 |
| `--nodata` | raw value meaning "no data" | none |
| `--min`, `--max` | index range spread across the color ramp (give both) | NDVI -0.2 to 0.9, NDWI -0.5 to 0.8 |
| `--threads` | worker threads | all available |

### Output

```
tiles/ndvi/
├── manifest.json
├── tile_r000_c000.png
├── tile_r000_c001.png
└── ...
```

These are **pixel-grid tiles**: tile `r000_c000` is the top-left block of the source image. They are not web-map (XYZ) tiles, which would need reprojection first. `manifest.json` records the image size, tile layout, color range, calibration, and statistics of the index.

Two different indices are both called "NDWI". This tool uses McFeeters' (green and NIR), which highlights open water. Gao's NDWI (NIR and shortwave infrared) measures vegetation moisture.

## Benchmarks

Compare the Rust code with NumPy on the same data (release mode matters):

```bash
cargo run --release -p spectral-core --example bench -- 30000000
python benchmark/numpy_baseline.py 30000000
```

Both compute the same thing: NDVI with an offset, no-data handling, and clamping. Fill in your own results, and note your CPU and core count:

| Implementation | Time (ms) | Mpixel/s |
| --- | --- | --- |
| NumPy | | |
| Rust, 1 thread | | |
| Rust, N threads | | |

Plain NumPy allocates several temporary arrays, so it represents typical NumPy rather than the fastest possible Python. Libraries such as numexpr or numba would narrow the gap, and a fair write-up says so.

## Tests

```bash
cargo test                       # everything
cargo test -p spectral-core      # the dependency-free core only
cargo clippy --all-targets
```

| Area | What is checked |
| --- | --- |
| Index math | NumPy reference values, scale invariance, the offset effect, no-data, zero and negative sums, clamping, band-swap symmetry |
| Parallelism | identical output to the single-threaded version for many thread counts, uneven chunks, more threads than pixels |
| Colormap | end colors, interpolation, clamping, transparency, invalid ramps and stretches |
| Tiling | exact fits, edge tiles, every pixel in exactly one tile, rendering reads the right pixels |
| Output | valid JSON with `null` instead of NaN, string escaping |
| End to end | synthetic GeoTIFFs in, correct tile count, sizes, colors, transparency, and statistics out |

## Limitations

- Each band is read whole, so a full Sentinel-2 band (about 120 million pixels) is the practical limit. Clip larger scenes first.
- Only single-band 8-bit or 16-bit unsigned GeoTIFFs are read.
- PNG tiles carry no georeferencing and are not web-map tiles.
- No cloud masking: clouds are bright in both bands and are colored like anything else.

## License

This project is released under the [MIT License](LICENSE).
