#!/bin/sh
# Regenerates every measurement: the CSVs and text tables in
# experiments/results/, the error figure, the tables of experiments/README.md
# and the table bodies and plot data the paper reads.
#
#   sh experiments/run.sh
#
# crossing_error and frames are deterministic; step_cost reports times and
# varies between runs. The stills and frame times of the worlds are
# paper/stills.sh, and reproduce.sh at the top of the repository runs both.
set -eu
cd "$(dirname "$0")/.."
out=experiments/results
mkdir -p "$out"

# The seeds of the confirmation runs, which the paper's numbers come from.
# Each program holds the seeds it was designed on as constants.
error_seeds="3c6ef372fe94f82b a54ff53a5f1d36f1"
frame_seeds="1f83d9abfb41bd6b 5be0cd19137e2179"

{
    echo "date:     $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "machine:  $(uname -srm)"
    case "$(uname -s)" in
        Darwin) echo "cpu:      $(sysctl -n machdep.cpu.brand_string)" ;;
        Linux) echo "cpu:      $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')" ;;
    esac
    echo "rustc:    $(rustc --version)"
    echo "cargo:    $(cargo --version)"
    echo "profile:  release, as set in Cargo.toml"
    echo "flags:    RUSTFLAGS=${RUSTFLAGS:-(unset)}"
    echo "seeds:    crossing_error confirmation $error_seeds"
    echo "seeds:    frames confirmation $frame_seeds"
    echo "seeds:    design seeds are constants at the top of each program"
    if git rev-parse --verify --quiet HEAD >/dev/null 2>&1; then
        echo "commit:   $(git rev-parse HEAD)"
        echo "modified: $(git status --porcelain | wc -l | tr -d ' ') files"
    else
        echo "commit:   none (no commit to name)"
    fi
} > "$out/environment.txt"
cat "$out/environment.txt"

cargo test --release -p groxel-experiments
cargo run --release -p groxel-experiments --bin crossing_error -- "$out/crossing_error.csv" | tee "$out/crossing_error.txt"
# The same experiment on seeds it was not designed on.
# shellcheck disable=SC2086
cargo run --release -p groxel-experiments --bin crossing_error -- "$out/crossing_error_confirm.csv" $error_seeds | tee "$out/crossing_error_confirm.txt"
cargo run --release -p groxel-experiments --bin step_cost -- "$out/step_cost.csv" | tee "$out/step_cost.txt"
cargo run --release -p groxel-experiments --bin sizes | tee "$out/sizes.txt"
cargo run --release -p groxel-experiments --bin frames -- "$out/frames.csv" | tee "$out/frames.txt"
# The confirmation run also saves its first view, for the paper's figure.
pictures=$(mktemp -d)
# shellcheck disable=SC2086
cargo run --release -p groxel-experiments --bin frames -- "$out/frames_confirm.csv" $frame_seeds "$pictures" | tee "$out/frames_confirm.txt"
mkdir -p paper/figures
if command -v sips >/dev/null; then
    sips -s format jpeg -s formatOptions 90 "$pictures/uncut.png" --out paper/figures/pixels_uncut.jpg >/dev/null
    sips -s format jpeg -s formatOptions 90 "$pictures/floor-1e-2_marked.png" --out paper/figures/pixels_marked.jpg >/dev/null
else
    echo "sips not found (it is part of macOS): paper/figures/pixels_*.jpg not rewritten"
fi
rm -rf "$pictures"
python3 experiments/figures.py
