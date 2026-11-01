"""NumPy baseline for the spectral index tiler benchmark.

Computes the same thing as the Rust code (NDVI with offset, no-data handling,
and clamping) on the same kind of data, so the timings are comparable.

    python benchmark/numpy_baseline.py 30000000

Plain NumPy allocates several temporary arrays, so this is a fair picture of
"typical NumPy", not of the fastest possible Python (numexpr or numba would
do better).
"""

import sys
import time

import numpy as np

OFFSET = np.float32(-1000.0)
NODATA = 0


def noisy(n, seed):
    """Plausible Sentinel-2 values below 6000 with a few zeros (no data)."""
    rng = np.random.default_rng(seed)
    return rng.integers(0, 6000, size=n, dtype=np.uint16)


def ndvi(first, second):
    a = first.astype(np.float32) + OFFSET
    b = second.astype(np.float32) + OFFSET
    total = a + b
    valid = (total > 0) & (first != NODATA) & (second != NODATA)
    out = np.full(first.shape, np.nan, dtype=np.float32)
    np.divide(a - b, total, out=out, where=valid)
    np.clip(out, -1.0, 1.0, out=out)
    return out


def best_of(runs, work):
    best = float("inf")
    for _ in range(runs):
        start = time.perf_counter()
        work()
        best = min(best, time.perf_counter() - start)
    return best


def main():
    pixels = int(sys.argv[1]) if len(sys.argv) > 1 else 30_000_000
    first, second = noisy(pixels, 1), noisy(pixels, 2)
    print(f"{pixels} pixels (best of 5 runs)\n")
    elapsed = best_of(5, lambda: ndvi(first, second))
    print(f"numpy:         {elapsed * 1e3:8.1f} ms  {pixels / elapsed / 1e6:8.1f} Mpixel/s")


if __name__ == "__main__":
    main()
