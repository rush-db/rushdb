#!/usr/bin/env python3
"""Convert the ann-benchmarks SIFT1M HDF5 to Rust-loadable fixtures.

Outputs (in the same directory as the input hdf5):
  base.fvecs       : 1M x 128-dim float32 base vectors
  query.fvecs      : 10k x 128-dim float32 query vectors
  neighbors.ivecs  : ground-truth: 10k rows x 100 int32 nearest-neighbor ids

This is the classic large unfiltered recall gate (no payload metadata; the
benchmark uses its own synthetic categories for the filtered experiment).
"""

import sys
import struct
import h5py


def write_fvecs(rows, path: str) -> None:
    with open(path, "wb") as f:
        for r in rows:
            f.write(struct.pack("<I", r.shape[0]))
            f.write(r.astype("<f4").tobytes())


def write_ivecs(rows, path: str) -> None:
    with open(path, "wb") as f:
        for r in rows:
            f.write(struct.pack("<I", len(r)))
            for v in r:
                f.write(struct.pack("<i", int(v)))


def main() -> int:
    src, out_dir = sys.argv[1], sys.argv[2]
    with h5py.File(src, "r") as h5:
        train = h5["train"][:]
        test = h5["test"][:]
        neighbors = h5["neighbors"][:]

    write_fvecs(train, f"{out_dir}/base.fvecs")
    write_fvecs(test, f"{out_dir}/query.fvecs")
    write_ivecs(neighbors, f"{out_dir}/neighbors.ivecs")
    print(f"  SIFT1M base {train.shape} -> {out_dir}/base.fvecs")
    print(f"  SIFT1M query {test.shape} -> {out_dir}/query.fvecs")
    print(f"  SIFT1M neighbors {neighbors.shape} -> {out_dir}/neighbors.ivecs")
    return 0


if __name__ == "__main__":
    sys.exit(main())