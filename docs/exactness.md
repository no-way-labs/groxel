# What "exact" means here, and why it holds

This is the claim the paper makes about crossing a seam, the argument for it, the code each
step of the argument rests on, and what the claim does not say. It refers to
`crates/groxel/src/trace.rs` by function name.

## Setting

A world is a set of rooms, each a box of the integer lattice, and a set of seams. A seam sends
the voxel beyond a face to `R c + o`, where `R` is one of the 48 signed permutation matrices and
`o` is an integer vector (`LatticeMap`).

Take a ray with origin `O`, direction `D` and starting voxel `c`, all in the frame of the room
it starts in. Two traces of it are compared.

- **The uncut trace.** The world is unrolled along the ray: each time the ray would cross a
  seam, the room beyond is laid down next to the current one in the starting frame. The ray is
  then traced in that one frame with no seam on the way, by this engine's own stepper: the
  expressions of `Cursor::new`, counted steps, and ties to the later axis. "Amanatides and
  Woo's algorithm" does not fix the bits, and this does.
- **The glued trace.** The engine's cursor in the world as built, crossing seams.

Three points make the uncut trace exact enough to check.

- **It is defined step by step, and needs no uncut world to exist.** The uncut voxel `v`
  corresponds to the glued voxel `M(v)`, where `M` composes the maps of the seams crossed so
  far. The test worlds do have one uncut world each (`CutWorld::whole` and `Blocks::whole` in
  the tests, `unrolled` in the experiments crate). A world with a seam on part of a face, a
  one-way link, a mirror, or rooms that would overlap when laid down has none, and the
  definition by induction still applies to it.
- **It is laid out in the frame of the starting room, at the starting voxel's coordinates.** A
  plain tracer can change its result when its world is turned, because ties go by axis number
  and an origin may round when it is written in another frame. A ray that starts in a turned
  room agrees with a turned copy of the original world, not with the original.
- **It is compared at every step**, not only where the ray stops.

Both use the same per-axis state: `step` (which way the ray goes along the axis), `t_delta` (the
distance between faces along it), `t_first` (the distance to the first face), `count` (faces
crossed so far) and `t_max` (the distance to the next face, always
`t_first + count * t_delta`). The glued trace also carries `rank`, the number each axis had in
the starting frame.

## Claim

For every ray, the glued trace takes the same number of steps as the uncut trace, and at each
step it reports the same distance, bit for bit, and is in the voxel that the seams' composed map
assigns to the uncut trace's voxel. Hence both end at the same step, with the same distance,
in corresponding voxels, having entered through corresponding faces.

Neither trace takes a step whose distance is infinite. A ray with no direction, or with
components too small to have a reciprocal, reaches no face, and `advance` returns `Left` for it
in both.

## Argument

Let `π` be the permutation from starting-frame axes to current-frame axes after the seams
crossed so far, and let `σ_i` be the sign the composed turn gives axis `i`. The invariant is:

    t_max_glued[π(i)]   = t_max_uncut[i]        (the same bits)
    t_delta_glued[π(i)] = t_delta_uncut[i]      (the same bits)
    t_first_glued[π(i)] = t_first_uncut[i]      (the same bits)
    count_glued[π(i)]   = count_uncut[i]
    rank_glued[π(i)]    = i
    step_glued[π(i)]    = σ_i * step_uncut[i]

1. **It holds at the start.** `Cursor::new` computes `step`, `t_delta` and `t_max` from `O`, `D`
   and `c`, sets `t_first` to `t_max`, `count` to zero and `rank` to `[0, 1, 2]`. Both traces start from the same values with `π` the
   identity.

2. **Both choose the same axis.** `next_axis` returns the least axis under the order "smaller
   `t_max` first; if equal, larger `rank` first". Ranks are distinct, and `t_max` is never NaN
   (`t_delta` is never negative and never a NaN, `Cursor::new` clamps `t_max` with `max(0.0)`,
   which discards a NaN, and the product in step 4 is formed only after `count` is at least 1),
   so this is a total order on the three axes. It reads only the pairs
   `(t_max, rank)`, never an axis number. By the invariant the glued pairs are the uncut
   pairs, so if the uncut trace chooses axis `i` the glued trace chooses `π(i)`. In the
   uncut trace `rank` is the axis number, which is the tie rule of a tracer that has no seams.

3. **Both report the same distance.** The distance of a step is the chosen axis's `t_max`, read
   before it is advanced. By the invariant these are the same bits.

4. **Both advance the same way.** Each then adds one to `count[axis]` and sets `t_max[axis]` to
   `t_first[axis] + count[axis] as f32 * t_delta[axis]` (`step_along`): one conversion, one
   multiplication and one addition in single precision, on the same operands, with the same
   result. This is the only floating-point arithmetic a step performs on the stepping state, at
   a seam or elsewhere.

5. **A seam preserves the invariant.** If the face crossed has a seam with turn `R`, `advance`
   re-reads the state in the new frame: for each new axis `j`, with `(src, sign) =
   R.source(j)`, it sets `step[j] = step[src] * sign` and copies `t_max[src]`, `t_delta[src]`,
   `t_first[src]`, `count[src]` and `rank[src]` to `j`. Nothing is recalculated. The invariant holds again with `π` followed
   by `R`'s permutation and `σ` multiplied by `R`'s signs.

6. **The voxels correspond.** The glued trace's voxel moves by `step` along the chosen axis and,
   at a seam, through the seam's integer map. Both are integer operations, so the voxel is
   the image, under the composed map, of the uncut trace's voxel. The two traces
   therefore read the same materials, stop at the same step, and by step 3 at the same distance.
   The chosen axis has a `step` of 1 or -1: an axis with a `step` of 0 has an infinite `t_max`,
   and `advance` returns before it would cross a face at infinity.

The stopping rules do not disturb this. `trace` stops when the next distance exceeds `max_t` or
is infinite, or when the step count reaches `max_steps`. With `skip` set it also looks past a
solid overlay voxel of one entity while the distance is below `skip.near`. All of these are
functions of quantities already shown equal: the distance, the step count, and what is in the
voxel.

The skip rule reads the distance and not the number of seams crossed, although a count of seams
would tell a viewer's own figure from its image more sharply. The uncut trace crosses no seams,
so a rule that read the count would stop the two traces in different places: a ray from one side
of a cut to a voxel of the viewer's on the other side would stop there in the cut world and go
on in the uncut one.

## What the claim rests on

- **The turn is a signed permutation.** Each axis of the old frame is one axis of the new, so
  per-axis quantities can move with their axis. A general rotation mixes axes, and the state
  would have to be recomputed. This is why the engine admits the 48 orientations and no others.
- **The tie rule travels with the ray.** A ray through a voxel edge reaches two faces at the
  same distance, and which is crossed first decides which voxel is visited. If ties were broken
  by current axis number, a seam, which renumbers the axes, would change the answer. A test
  found this before `rank` was added: a ray visited 9 voxels in the cut world and 10 in
  the uncut one.
- **Arithmetic is IEEE 754 single precision, one operation at a time.** Rust does not contract
  `a * b + c` into a fused multiply-add and does not reorder floating-point operations, so both
  traces execute the same additions. A target that keeps `f32` values in wider registers (x87)
  is outside the argument and outside the tests.

## What the claim does not say

- **It is not a claim of accuracy.** The uncut trace is itself a single-precision computation,
  and the glued trace reproduces its error, whatever that is. With the usual running total
  (`t_max += t_delta`) the error grows with the square of the distance: a median of 0.09 voxels
  at 5,400 voxels, with 14% of rays ending on another surface (`experiments/README.md`). A
  permuting trace in that form agrees with its uncut trace bit for bit and is less accurate than
  a tracer that recomputes its state at each seam. This is why the cursor counts its steps: the
  median error at 5,400 voxels is then 0.0002 voxels, as small as that of the best recomputing
  method measured. In rays that end on another surface the engine has 6 of 20,000 there, and
  the methods measured have 0 to 11. The argument above does not depend on the form. It goes through for a running total
  with step 4 replaced by the one addition, and the experiments crate checks both forms.
- **It is not a claim about the hit point.** The cursor also carries the ray's origin and
  direction into each new frame (`origin = map.point(origin)`), for callers that want the hit
  point in the frame of the voxel hit. That is floating-point arithmetic and rounds. The
  traversal never reads it. The tests allow the carried point 0.0001 voxels at one crossing.
- **It is not a claim about shading.** A renderer computes on the hit point. In the frame test
  (`groxel-render/tests/render.rs`) no pixel of 248,832 differed from the uncut world's frame,
  but nothing in the argument above guarantees that, and the test itself allows a frame 12
  pixels that differ by more than one level.

## Evidence

Every test below compares the cut world with the uncut one. "Equal" for a distance means the
same bits.

| What | Where | What is compared |
|---|---|---|
| One crossing, each of the 48 turns, 400 random rays each | `rays.rs`, `cut_turn_and_glue_shows_the_same_picture_all_48_ways` | How the trace ended, distance, step count and last voxel for every ray; material, face and frame for rays that stop on a solid. Again with a limit on distance and with a limit on steps |
| One crossing, each of the 48 turns, the 26 lattice directions from voxel centres | `rays.rs`, `rays_through_edges_and_corners_agree_all_48_ways` | How the trace ended, distance and step count; voxel and material for solid hits. No faces |
| A cube cut into 125 blocks, each turned at random, 4 worlds, 7,096 rays each, a dozen seams on a ray | `rays.rs`, `every_step_agrees_through_many_turned_blocks` | At every step: distance, voxel, entry face, frame and material. Random rays, and lattice rays with components from -2 to 2 from centres and corners |
| A ray that reaches no face, beside a seam and not | `rays.rs`, `a_ray_that_reaches_no_face_stays_put_with_or_without_a_seam` | The whole result |
| Rays that look past a viewer's sprite lying across the cut, each of the 48 turns | `overlay.rs`, `a_sprite_across_a_turned_seam_matches_the_uncut_world_all_48_ways` | How the trace ended, distance, step count, material and entity |
| Up to 1,000 crossings in the quarter-turn space, 200,000 rays that cross a seam and 20,000 that do not | `experiments`, `crossing_error` | The final hit only, for running totals and for counted steps |
| The same at 3, 40 and 300 crossings, in the test suite | `experiments/src/lib.rs` | The final hit only |

The two experiments rows compare where a ray stops, after a gate that ignores solid voxels on
the way. The order of two tied faces rarely changes that, and the quarter-turn space reaches
only the four frames that turn about one axis. With ties broken by the current axis number, the experiment
still finds the engine equal to its uncut trace on 219,998 of 220,000 rays. Those rows are
evidence for the float half of the claim over many crossings, and they are weak evidence for the
tie rule. The blocks test is the one that holds the tie rule after many seams: it fails with
ties broken that way, as does the lattice-direction test. Each test in the table was checked to fail when
the thing it guards is broken.

## Limits

- The tests have run on `aarch64-apple-darwin` only. The argument assumes IEEE 754 single
  precision with one operation at a time, which holds on x86-64 with SSE and on WebAssembly,
  and neither has been run.
