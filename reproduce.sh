#!/bin/sh
# Regenerates every number, table and figure of the paper and the READMEs,
# then builds the paper.
#
#   sh reproduce.sh
#
# Needs cargo and python3. The stills are converted to JPEG with `sips`,
# which is part of macOS, and the paper is built if `latexmk` is installed.
# The machine, compiler, flags, seeds and commit are recorded in
# experiments/results/environment.txt. Under ten minutes on an Apple M3.
set -eu
cd "$(dirname "$0")"
sh experiments/run.sh
sh paper/stills.sh
if command -v latexmk >/dev/null; then
    (cd paper && latexmk -pdf -interaction=nonstopmode -halt-on-error groxel.tex)
else
    echo "latexmk not found: the paper was not built"
fi
