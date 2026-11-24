# Experiments

This directory holds the programs that produce the paper's numbers, the baselines they compare
the engine with, and their results.

`sh experiments/run.sh` runs every measurement. It writes the CSVs and the error figure to
`results/`, the tables of this file, and the table bodies and plot data that the paper reads,
and it records the machine, compiler, flags, seeds and commit in `results/environment.txt`.
`sh reproduce.sh` at the top of the repository runs it, then renders the stills and times the
four worlds, then builds the paper. Each program also runs alone:

```
cargo run --release -p groxel-experiments --bin crossing_error -- experiments/results/crossing_error.csv
cargo run --release -p groxel-experiments --bin step_cost      -- experiments/results/step_cost.csv
cargo run --release -p groxel-experiments --bin frames         -- experiments/results/frames.csv
cargo run --release -p groxel-experiments --bin sizes
python3 experiments/figures.py
cargo test -p groxel-experiments
```

The results here are from one machine and one compiler: an Apple M3 with 24 GB, macOS 15.8.2
and rustc 1.99.0, in the release profile on one thread. Each program holds the seeds it was
designed on as constants and takes others on its command line. `crossing_error` and `frames`
are deterministic, and `step_cost` reports times whose crossing costs moved by up to 5 ns
between runs.

`figures.py` writes the tables of this file from the run on the design seeds, and the paper's
tables from a second run on other seeds (`*_confirm.csv`), so that no number in the paper comes
from the seeds the experiments were designed on.

## The world

The test world is a cube of 8 voxels a side with every face glued to the opposite one, x and y
by plain wraps and z with a quarter turn, and with 15% of its voxels solid. It is the
quarter-turn space with a voxel lattice in it. A ray never leaves it, and a measurement can
therefore choose how many seams the ray crosses.

Each ray passes through its first `c` seams unobstructed and then stops at the first solid
voxel. The same ray is traced in the unrolled world (all of space tiled with turned copies of
the cube), which has no seams. A voxel of the unrolled world is looked up by folding its integer
coordinates back into the cube, and a test checks the folding against the engine's lattice walk.

## What is compared

| Name | What it does at a seam | After |
|---|---|---|
| `engine` | The engine's cursor. The stepping state moves to the axes the seam's turn sends it to, and nothing is recalculated. The distance to the next face is `first + count * delta` (counted steps) | this engine |
| `perm-count` | The same, written in this crate against the public API | |
| `perm-run` | The same rule with the usual running total, `tMax += tDelta` | |
| `recompute` | The crossing point and direction are transformed, the voxel goes through the seam's integer map, the distance restarts, and step, tMax and tDelta are worked out again | boxspace, which carries an integer cube index and recomputes the exit face in every cube |
| `fishpye` | The origin and direction are transformed, one running distance is kept, the voxel is the truncated point at that distance, and step, tDelta and tMax are reset | fishpye, `raytrace.cl` lines 313 to 343 at commit `8e9cbef` |
| `floor-ε` | As `recompute`, but the point is first moved ε along the ray and the voxel is the floor of the transformed point | the rule of noeuclid, `Shaders/Pass1.frag` at commit `b2ab611`, where ε is 0.01 |
| `cell-matrix` | What it does at every wall of every cell. The ray is a position and a tangent in the coordinates of its cell. The distance to each of the six walls is computed, the position moves to the nearest wall, and a matrix takes position and tangent into the next cell's coordinates. Distance is a sum of the wall-to-wall distances | HyperRogue, the Euclidean branch of the shader built in `raycaster.cpp`, at commit `54a3125` |

The baselines are our re-implementations in this world, written from the sources of the
programs they stand for, and they differ from those programs in three ways.

- fishpye's portals are blocks with volume and any 4x4 matrix, whereas a seam here is a face
  whose map is a signed permutation with an integer shift. When fishpye's truncated point falls
  outside the room, `fishpye` moves it to the nearest voxel inside, which repairs a point that
  fell short of the face. In fishpye's own grid that point is a real voxel, one behind the right
  one.
- noeuclid keeps no stepping state: at every step it moves a point 0.01 past the boundary and
  takes the floor. `floor-ε` applies that rule at seams only, and therefore understates what
  noeuclid loses.
- HyperRogue's shader declares its numbers `mediump`, to which desktop OpenGL gives 32 bits, and
  `cell-matrix` runs in 32 bits. A device that gives `mediump` 16 bits is not measured. The
  shader's expressions are kept in their order, the matrices here have entries 0, 1 and -1, and
  the shader uses its distance for fog only.

Each method is compared with two references.

- **Uncut.** The unrolled world in single precision, with the same form of stepping state, which
  is what an engine with no seams computes.
- **True.** The unrolled world in double precision, whose rounding error at these distances is
  below 1e-9 voxels and which stands for the true ray.

## Result 1: a permuting trace is the uncut trace

The table gives the rays, of 20,000 at each crossing count, whose result equals the uncut trace
bit for bit in voxel, face, material, distance and number of voxels visited. Each permuting
method is compared with the uncut trace in its own form of stepping state, and the baselines
with the engine's.

<!-- agreement: written by figures.py -->
| Seams crossed | 1 | 10 | 100 | 1,000 |
|---|---|---|---|---|
| `engine` | 20,000 | 20,000 | 20,000 | 20,000 |
| `perm-count` | 20,000 | 20,000 | 20,000 | 20,000 |
| `perm-run` | 20,000 | 20,000 | 20,000 | 20,000 |
| `recompute` | 12,053 | 8,956 | 3,662 | 786 |
| `fishpye` | 14,026 | 9,919 | 10,199 | 8,897 |
| `cell-matrix` | 9,426 | 3,457 | 796 | 171 |
| `floor-1e-3` | 10,370 | 8,563 | 3,453 | 346 |
<!-- end agreement -->

The three permuting methods equal their uncut traces on all 220,000 rays of the full table,
which has 11 crossing counts. This is the cut, turn and re-glue test extended from one crossing
to a thousand, and a test in this crate keeps it true.

Because `cell-matrix` keeps no stepping state, its distances differ from the uncut trace's in
the last bits with or without seams, and it equals the uncut trace on 9,047 of 20,000 rays that
cross no seam.

## Result 2: accuracy on long rays

Each method is compared with the true ray. "Off" is the number of rays, of 20,000, that end on
a different voxel or face from the true ray, and "error" is the median distance error, in
voxels, of the rays that end on the true surface.

<!-- accuracy: written by figures.py -->
| Seams crossed (mean distance) | 1 (7) | 10 (55) | 100 (540) | 1,000 (5,398) |
|---|---|---|---|---|
| `engine`: off | 0 | 0 | 0 | 6 |
| `engine`: error | 1.6e-7 | 1.6e-6 | 1.7e-5 | 1.8e-4 |
| `perm-run`: off | 0 | 0 | 44 | 2,883 |
| `perm-run`: error | 2.0e-7 | 1.2e-5 | 1.1e-3 | 8.7e-2 |
| `recompute`: off | 0 | 0 | 0 | 0 |
| `recompute`: error | 2.1e-7 | 2.2e-6 | 6.7e-5 | 4.3e-3 |
| `fishpye`: off | 0 | 0 | 0 | 11 |
| `fishpye`: error | 1.8e-7 | 2.2e-6 | 2.3e-5 | 2.4e-4 |
| `cell-matrix`: off | 0 | 0 | 0 | 5 |
| `cell-matrix`: error | 2.2e-7 | 6.4e-6 | 3.5e-4 | 2.9e-2 |
| `floor-1e-3`: off | 7 | 3 | 7 | 8 |
| `floor-1e-2`: off | 68 | 67 | 66 | 84 |
<!-- end accuracy -->

![Median distance error against seams crossed](results/crossing_error.svg)

With a running total the error grows with the square of the distance, because the rounding of
the addition `tMax += tDelta` has a constant bias while tMax stays in one binade. Seams play no
part in it: the uncut trace with a running total has the same error, bit for bit. At 1,000
crossings (5,398 voxels) the median error is 0.087 voxels and 2,883 of 20,000 rays end on
another surface.

Every recomputing method works its distances out again at each seam and does not accumulate
this error. On long rays a permuting trace with a running total is therefore less accurate than
any of them, although a crossing adds no rounding to it.

Counted steps remove the drift without recomputing: at 1,000 crossings the engine's median
error is 1.8e-4 voxels against 2.4e-4 for `fishpye`, the most accurate baseline, and the engine
equals its uncut trace bit for bit, which is why its cursor counts its steps.

`cell-matrix` finds the surface as reliably as any method here: no ray in 20,000 is off up to
100 crossings, and 5 are off at 1,000. Its position is never more than a voxel from the origin
of its coordinates and retains its precision at any distance. Its distance is a sum of the
wall-to-wall distances and drifts as the running total does, to 2.9e-2 voxels at 1,000
crossings, which is a third of `perm-run`'s error and 160 times the engine's. HyperRogue uses
that distance for fog only. The method computes six wall distances in every cell.

The `floor` variants put a ray in the wrong voxel when it crosses a seam close to a voxel edge:
2 to 9 rays of 20,000 with a nudge of 0.001 and 62 to 84 with 0.01, at each crossing count from
1 to 1,000. The rate does not depend on the number of crossings, because only a ray's last few
crossings come after the gate.

## Result 3: cost

The time per step in empty quarter-turn spaces of side 4 to 64 is fitted as
`plain step + crossing rate × extra`.

<!-- cost: written by figures.py -->
| | Plain step (ns) | Extra per crossing (ns) |
|---|---|---|
| `engine` | 7.3 | 25 |
| `perm-count` | 7.5 | 26 |
| `perm-run` | 6.7 | 24 |
| `recompute` | 6.9 | 42 |
| `fishpye` | 6.8 | 43 |
| `floor-1e-2` | 6.8 | 41 |
<!-- end cost -->

A recomputing crossing costs 16 to 18 ns more than a permuting one in this run (three divisions
and the transform of a point), and 17 to 19 ns in another. More than half of its cost is common
to every method: finding the seam and mapping the voxel. Counted steps cost 0.7 ns more per
plain step than a running total, measured on the two loops in this crate (7.5 against 6.7 ns).

`cell-matrix` is a method for a GPU shader and is not timed, because a loop of it on a CPU would
time this crate's transcription of it.

## Result 4: the link test

The measurement uses a room of side 128 with no seams and 40,000 rays from its middle, 4.7
million steps in all. The cursor's loop is copied into `step_cost` over a bare array of the
engine's 16-bit cells and timed with the test of a face's link bit compiled in and with it left
out, with nothing else changed. The loops alternate for 31 repeats each.

<!-- link test: written by figures.py -->
| | Fastest (ns a step) | Median (ns a step) |
|---|---|---|
| The cursor's loop, copied, without the link test | 4.67 | 4.69 |
| The same loop with the link test | 4.83 | 4.84 |
| The engine's cursor | 7.59 | 7.64 |
<!-- end link test -->

The link test costs 0.16 ns of a 4.83 ns step by the fastest repeats and 0.15 ns by the medians,
which is 3%, and five other runs of the program gave 0.08 to 0.16 ns.

The engine's cursor takes 7.59 ns for the same step. The 2.76 ns it adds to the copied loop
come from elsewhere: the cursor looks its room up by id on every step, returns a value that
names the face and the seam, checks for an overlay when it reads a material, refuses a face at
infinite distance, and is called across a crate boundary. The separate costs of these are not
measured.

Two copies of one loop are compared because the form of a loop changes its time more than the
test does: hand-written loops of other forms took longer than the cursor.

## Result 5: memory

| | Bytes |
|---|---|
| A voxel in the core | 2 (material, link bits) |
| A voxel's light | 2 (lamp, sky) |
| A voxel in an overlay | 1 bit, and a hash-map entry if something is stamped there |
| A seam | 56, and 4 for its entry in its room's list |
| A cursor | 136 |

The cost of a seam does not depend on its size. A table with one entry per linked face would
hold 31,280 entries for the Tower's cut, which here is two seam records.

## Result 6: pictures

`frames` draws 8 views of 640 by 360 pixels in the quarter-turn space with 2% of its voxels
solid, one ray a pixel, from random places and in random directions. A view shows the same
solid voxels repeated into the distance through the seams, and a ray crosses 8.4 seams on
average and at most 501. The colour of a pixel comes from the material and the face the ray
stopped on, faded with distance and rounded to 8 bits a channel. Each method's pictures are
compared pixel by pixel with the pictures its uncut trace draws, 1,843,200 pixels in all.

<!-- pixels: written by figures.py -->
| | Pixels that differ | On another surface | Most in one view |
|---|---|---|---|
| `engine` | 0 | 0 | 0 |
| `perm-run` | 0 | 0 | 0 |
| `recompute` | 16 | 8 | 4 |
| `fishpye` | 14 | 10 | 5 |
| `cell-matrix` | 18 | 9 | 6 |
| `floor-1e-3` | 164 | 160 | 32 |
| `floor-1e-2` | 1,511 | 1,489 | 313 |
<!-- end pixels -->

"On another surface" counts the differing pixels whose ray stopped on another voxel or face.
The other differing pixels stopped on the same surface at a distance that rounds to another
colour.

The engine's pictures equal the uncut pictures in every pixel. `recompute`, `fishpye` and
`cell-matrix` each differ in 14 to 18 pixels, fewer than one in 100,000, although 45 to 71% of
their rays differ from the uncut trace in some bit. The `floor` variants differ in 164 and 1,511
pixels, up to 313 in one view, and 1,649 of those 1,675 pixels are on another surface, as single
pixels along voxel edges. The run on other seeds gave 0 pixels for the engine, 6 to 23 for the
three baselines, and 254 and 2,279 for the `floor` variants.

The shading is a flat one written in this crate, which reads no hit point and casts no shadow
rays. The engine's own renderer, with light, shadows and outlines, is compared with the uncut
world in `crates/groxel-render/tests/render.rs`, for the engine only.

## Summary

- The engine agrees with the uncut trace bit for bit over a thousand crossings. No baseline
  does: the best, `fishpye`, agrees on 8,897 of 20,000 rays at 1,000 crossings.
- Agreement is a different property from accuracy, and a permuting trace needs counted steps to
  have both. With a running total it agrees with its uncut trace and is the least accurate
  method measured.
- With counted steps the engine's distance is as accurate as the best baseline's, and a crossing
  costs 16 to 19 ns less than a recomputing one.
- A matrix at every cell wall, HyperRogue's rule, finds the right voxel as reliably as any
  method here, and its distance drifts. The engine has both properties for the cost of a link
  test, 0.08 to 0.16 ns a step.
- In pictures the three careful baselines differ from the uncut world in fewer than one pixel in
  100,000 and the engine in none. Nudge-and-floor crossing differs in up to 1,511 of 1,843,200
  pixels.
