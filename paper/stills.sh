#!/bin/sh
# Renders the paper's stills and measures frame times.
#
#   sh paper/stills.sh
#
# Writes paper/figures/{gallery,mirror,well,tower,hall}.jpg, the same five
# pictures to docs/, and paper/tables/frames.tex. JPEG conversion uses
# `sips`, which is part of macOS.
set -eu
cd "$(dirname "$0")/.."
cargo build --release -p groxel-app
# Through cargo, so that a target directory set elsewhere is found.
still="cargo run --release -q -p groxel-app --bin still --"
tmp=$(mktemp -d)
mkdir -p paper/figures paper/tables experiments/results

# name, spot, arguments. Times are in the well, where a camera falls, and
# figures walk, so each shot fixes its moment with t.
shots="
gallery|gallery|t=3
mirror|gallery|t=3 yaw=150 dx=-6 dz=6
well|well|t=0.5 dx=22 dz=0.4 yaw=20 pitch=48
tower|overlook|t=4 dy=-20
hall|hall|t=3
"

# The median of the render times of five runs, in milliseconds.
median_ms() {
    for _ in 1 2 3 4 5; do
        # shellcheck disable=SC2086
        $still "$1" "$tmp/time.png" $2 | sed 's/.*rendered in \([0-9.]*\) ms.*/\1/'
    done | sort -n | sed -n 3p
}

: > experiments/results/frame_times.txt
: > paper/tables/frames.tex
echo "$shots" | while IFS='|' read -r name spot args; do
    [ -z "$name" ] && continue
    # shellcheck disable=SC2086
    $still "$spot" "$tmp/$name.png" w=1280 h=720 ss=2 $args
    sips -s format jpeg -s formatOptions 88 "$tmp/$name.png" --out "paper/figures/$name.jpg" >/dev/null
    cp "paper/figures/$name.jpg" "docs/$name.jpg"

    one=$(median_ms "$spot" "w=640 h=360 ss=1 threads=1 $args")
    all=$(median_ms "$spot" "w=640 h=360 ss=1 threads=0 $args")
    echo "$name: $one ms on one thread, $all ms on all cores (640 x 360, one sample a pixel)" |
        tee -a experiments/results/frame_times.txt
    label=$(echo "$name" | awk '{print toupper(substr($0,1,1)) substr($0,2)}')
    printf '%s & %.0f & %.0f \\\\\n' "$label" "$one" "$all" >> paper/tables/frames.tex
done
rm -rf "$tmp"
