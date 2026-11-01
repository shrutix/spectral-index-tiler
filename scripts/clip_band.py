"""Clip a window out of a (possibly remote) single-band GeoTIFF.

A full Sentinel-2 band is about 240 MB. Cloud-optimized GeoTIFFs let you read
just a window over HTTP, so you download only what you need.

    pip install rasterio
    python scripts/clip_band.py <band-url-or-path> red.tif --x 4000 --y 4000 --size 2048

The output is an uncompressed single-band GeoTIFF with the same data type,
coordinate system, and georeferencing as the source, which is what
spectral-tiler reads.

NOTE: this script has not been run against live data by its author.
"""

import argparse

import rasterio
from rasterio.windows import Window


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("source", help="URL or path of the source GeoTIFF")
    parser.add_argument("out", help="path of the clipped GeoTIFF to write")
    parser.add_argument("--x", type=int, default=0, help="left edge of the window, in pixels")
    parser.add_argument("--y", type=int, default=0, help="top edge of the window, in pixels")
    parser.add_argument("--size", type=int, default=2048, help="window width and height, in pixels")
    args = parser.parse_args()

    with rasterio.open(args.source) as src:
        width = min(args.size, src.width - args.x)
        height = min(args.size, src.height - args.y)
        if width <= 0 or height <= 0:
            raise SystemExit(f"The window starts outside the {src.width}x{src.height} image.")

        window = Window(args.x, args.y, width, height)
        data = src.read(1, window=window)
        profile = dict(
            driver="GTiff",
            dtype=data.dtype,
            count=1,
            width=width,
            height=height,
            crs=src.crs,
            transform=src.window_transform(window),
            compress="none",
        )
        if src.nodata is not None:
            profile["nodata"] = src.nodata

    with rasterio.open(args.out, "w", **profile) as dst:
        dst.write(data, 1)
    print(f"wrote {args.out}: {width}x{height} {data.dtype}")


if __name__ == "__main__":
    main()
