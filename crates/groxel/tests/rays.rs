//! Rays. The central test: a world cut in two, with one half turned any of
//! the 48 ways and glued back, shows a ray the same thing as the uncut world.

use groxel::{
    Cell, Cursor, Face, FaceRect, HitKind, IVec3, Moved, Orientation, Ray, RoomId, TraceOpts,
    World, ivec3, trace, vec3, visit,
};
use std::ops::ControlFlow;

mod common;
use common::{CutWorld, Rng};

#[test]
fn straight_ray_in_a_plain_room() {
    let mut w = World::new();
    let r = w.add_room("box", ivec3(10, 4, 4));
    w.room_mut(r).set(ivec3(7, 1, 1), 3);
    let hit = trace(
        &w,
        Ray::new(r, vec3(1.5, 1.5, 1.5), vec3(1.0, 0.0, 0.0)),
        &TraceOpts::default(),
    );
    assert_eq!(hit.kind, HitKind::Solid);
    assert_eq!(hit.material, 3);
    assert_eq!(hit.face, Face::NegX);
    assert!((hit.t - 5.5).abs() < 1e-5);
    assert_eq!(hit.from.at, ivec3(6, 1, 1));
    assert_eq!(hit.via, None);
    assert_eq!(hit.point(), vec3(7.0, 1.5, 1.5));

    let up = trace(
        &w,
        Ray::new(r, vec3(1.5, 1.5, 1.5), vec3(0.0, 1.0, 0.0)),
        &TraceOpts::default(),
    );
    assert_eq!(up.kind, HitKind::Sky);
    assert!((up.t - 2.5).abs() < 1e-5);

    let short = trace(
        &w,
        Ray::new(r, vec3(1.5, 1.5, 1.5), vec3(1.0, 0.0, 0.0)),
        &TraceOpts {
            max_t: 3.0,
            ..Default::default()
        },
    );
    assert_eq!(short.kind, HitKind::Far);
}

#[test]
fn a_wrapped_room_shows_the_back_of_the_pillar_behind_you() {
    let mut w = World::new();
    let r = w.add_room("ring", ivec3(8, 3, 3));
    w.wrap(r, 0).unwrap();
    w.room_mut(r).set(ivec3(1, 1, 1), 9);
    let hit = trace(
        &w,
        Ray::new(r, vec3(4.5, 1.5, 1.5), vec3(1.0, 0.0, 0.0)),
        &TraceOpts::default(),
    );
    assert_eq!(
        (hit.kind, hit.material, hit.crossings),
        (HitKind::Solid, 9, 1)
    );
    assert!((hit.t - 4.5).abs() < 1e-5);
}

#[test]
fn cut_turn_and_glue_shows_the_same_picture_all_48_ways() {
    let n = 6;
    let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15);
    let mut solid_hits = 0;
    let mut far_side_hits = 0;
    let mut cut_short_beyond = 0;

    for (ti, turn) in Orientation::all().enumerate() {
        let mut cw = CutWorld::new(n, turn);
        // Scatter solids, keeping a row of the left half clear to start from.
        for z in 0..n {
            for y in 0..n {
                for x in 0..2 * n {
                    let start_row = y == n / 2 && x > 0 && x < n - 1;
                    if rng.unit() < 0.12 && !start_row {
                        cw.set(ivec3(x, y, z), rng.below(1, 200) as u8);
                    }
                }
            }
        }

        for k in 0..400 {
            let cell = ivec3(rng.below(1, n - 1), n / 2, rng.below(0, n));
            if cw.whole.room(cw.whole_room).is_solid(cell) {
                continue;
            }
            let origin = vec3(
                cell[0] as f32 + 0.1 + 0.8 * rng.unit(),
                cell[1] as f32 + 0.1 + 0.8 * rng.unit(),
                cell[2] as f32 + 0.1 + 0.8 * rng.unit(),
            );
            let dir = rng.direction();
            let opts = TraceOpts::default();
            let want = trace(&cw.whole, Ray::new(cw.whole_room, origin, dir), &opts);
            let got = trace(&cw.cut, Ray::new(cw.left, origin, dir), &opts);
            let ctx = format!("turn #{ti} {turn:?}, ray {k}");

            assert_eq!(want.kind, got.kind, "{ctx}");
            // Exactly equal: crossing a seam does no arithmetic on distances.
            assert_eq!(want.t.to_bits(), got.t.to_bits(), "{ctx}");
            assert_eq!(want.steps, got.steps, "{ctx}");
            // However the trace ended, it ended in the same voxel.
            let (room, at) = cw.cell_in_cut(want.cell.at);
            assert_eq!(got.cell, Cell { room, at }, "{ctx}");

            // A limit on distance or on steps stops both in the same place.
            let limits = [
                TraceOpts {
                    max_t: 1.0 + 6.0 * rng.unit(),
                    ..Default::default()
                },
                TraceOpts {
                    max_steps: rng.below(1, 12) as u32,
                    ..Default::default()
                },
            ];
            for opts in &limits {
                let want = trace(&cw.whole, Ray::new(cw.whole_room, origin, dir), opts);
                let got = trace(&cw.cut, Ray::new(cw.left, origin, dir), opts);
                assert_eq!(want.kind, got.kind, "{ctx}, {opts:?}");
                assert_eq!(want.t.to_bits(), got.t.to_bits(), "{ctx}, {opts:?}");
                assert_eq!(want.steps, got.steps, "{ctx}, {opts:?}");
                let (room, at) = cw.cell_in_cut(want.cell.at);
                assert_eq!(got.cell, Cell { room, at }, "{ctx}, {opts:?}");
                if want.kind == HitKind::Far && got.crossings > 0 {
                    cut_short_beyond += 1;
                }
            }

            if want.kind != HitKind::Solid {
                continue;
            }
            solid_hits += 1;
            assert_eq!(want.material, got.material, "{ctx}");
            if want.cell.at[0] < n {
                assert_eq!(got.face, want.face, "{ctx}");
                assert_eq!(got.frame, Orientation::IDENTITY, "{ctx}");
            } else {
                far_side_hits += 1;
                assert_eq!(got.face, turn.face(want.face), "{ctx}");
                assert_eq!(got.frame, turn, "{ctx}");
                // The carried ray still passes through the hit point.
                let want_point = cw.point_in_right(want.point());
                let got_point = got.point();
                for i in 0..3 {
                    assert!(
                        (want_point[i] - got_point[i]).abs() < 1e-4,
                        "{ctx}: {want_point:?} vs {got_point:?}"
                    );
                }
            }
        }
    }
    // The test means nothing unless rays did cross the seam and hit things.
    assert!(solid_hits > 5000, "{solid_hits}");
    assert!(far_side_hits > 500, "{far_side_hits}");
    assert!(cut_short_beyond > 200, "{cut_short_beyond}");
}

#[test]
fn a_mirror_shows_what_is_behind_you() {
    let mut w = World::new();
    let r = w.add_room("m", ivec3(8, 3, 3));
    w.mirror(FaceRect {
        room: r,
        min: ivec3(7, 0, 0),
        max: ivec3(7, 2, 2),
        face: Face::PosX,
    })
    .unwrap();
    w.room_mut(r).set(ivec3(0, 1, 1), 5);
    let hit = trace(
        &w,
        Ray::new(r, vec3(4.5, 1.5, 1.5), vec3(1.0, 0.0, 0.0)),
        &TraceOpts::default(),
    );
    assert_eq!((hit.kind, hit.material), (HitKind::Solid, 5));
    // 3.5 to the glass, then 7 back to the near face of the far wall.
    assert!((hit.t - 10.5).abs() < 1e-4);
    assert!(hit.frame.is_reflection());
    assert_eq!(hit.face, Face::PosX);
}

#[test]
fn visit_sees_every_voxel_on_the_way() {
    let mut w = World::new();
    let r = w.add_room("ring", ivec3(6, 3, 3));
    w.wrap(r, 0).unwrap();
    w.room_mut(r).set(ivec3(2, 1, 1), 4); // visit does not stop for this
    let mut seen = Vec::new();
    let mut crossed = 0;
    visit(
        &w,
        Ray::new(r, vec3(0.5, 1.5, 1.5), vec3(1.0, 0.0, 0.0)),
        9.0,
        |cur, moved| {
            seen.push(cur.cell().at[0]);
            if let Moved::Entered { via: Some(_), .. } = moved {
                crossed += 1;
            }
            ControlFlow::Continue(())
        },
    );
    assert_eq!(seen, [1, 2, 3, 4, 5, 0, 1, 2, 3]);
    assert_eq!(crossed, 1);

    // Breaking stops it.
    let mut count = 0;
    visit(
        &w,
        Ray::new(r, vec3(0.5, 1.5, 1.5), vec3(1.0, 0.0, 0.0)),
        100.0,
        |cur, _| {
            count += 1;
            if cur.material() != 0 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    );
    assert_eq!(count, 2);
}

/// Rays that pass exactly through voxel edges and corners. Two faces are then
/// reached at the same distance, and which is crossed first decides which
/// voxel the ray visits. A seam renumbers the axes, so the choice must travel
/// with the ray or the cut world disagrees with the whole one.
#[test]
fn rays_through_edges_and_corners_agree_all_48_ways() {
    let n = 6;
    let mut rng = Rng::new(0x51ED_270B_0F1E_55AA);
    let mut ties_that_hit = 0;

    for (ti, turn) in Orientation::all().enumerate() {
        let mut cw = CutWorld::new(n, turn);
        for z in 0..n {
            for y in 0..n {
                for x in 0..2 * n {
                    let start_row = y == n / 2 && x > 0 && x < n - 1;
                    if rng.unit() < 0.2 && !start_row {
                        cw.set(ivec3(x, y, z), rng.below(1, 200) as u8);
                    }
                }
            }
        }

        for z in 0..n {
            for x in 1..n - 1 {
                // From a voxel's centre, along every diagonal of the lattice.
                let origin = ivec3(x, n / 2, z).centre();
                for d in 0..27 {
                    let dir = vec3(
                        (d % 3) as f32 - 1.0,
                        (d / 3 % 3) as f32 - 1.0,
                        (d / 9) as f32 - 1.0,
                    );
                    if dir == vec3(0.0, 0.0, 0.0) {
                        continue;
                    }
                    let opts = TraceOpts::default();
                    let want = trace(&cw.whole, Ray::new(cw.whole_room, origin, dir), &opts);
                    let got = trace(&cw.cut, Ray::new(cw.left, origin, dir), &opts);
                    let ctx = format!("turn #{ti} {turn:?}, from {origin:?} along {dir:?}");

                    assert_eq!(want.kind, got.kind, "{ctx}");
                    // Not merely close: a seam adds no arithmetic to the distance.
                    assert_eq!(want.t.to_bits(), got.t.to_bits(), "{ctx}");
                    assert_eq!(want.steps, got.steps, "{ctx}");
                    if want.kind == HitKind::Solid {
                        let (room, at) = cw.cell_in_cut(want.cell.at);
                        assert_eq!(got.cell, Cell { room, at }, "{ctx}");
                        assert_eq!(want.material, got.material, "{ctx}");
                        if got.crossings > 0 {
                            ties_that_hit += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(ties_that_hit > 500, "{ties_that_hit}");
}

/// A cube of `k * s` voxels a side cut into `k^3` blocks of side `s`. Each
/// block is its own room, turned its own way, and every pair of neighbouring
/// blocks is glued. The middle block is not turned, so a ray that starts in
/// it reads the same in both worlds.
struct Blocks {
    whole: World,
    whole_room: RoomId,
    cut: World,
    k: i32,
    s: i32,
    rooms: Vec<RoomId>,
    turns: Vec<Orientation>,
    shifts: Vec<IVec3>,
}

impl Blocks {
    fn new(k: i32, s: i32, rng: &mut Rng) -> Blocks {
        let orients: Vec<Orientation> = Orientation::all().collect();
        let mut whole = World::new();
        let whole_room = whole.add_room("whole", ivec3(k * s, k * s, k * s));
        let mut cut = World::new();
        let far = ivec3(s - 1, s - 1, s - 1);
        let (mut rooms, mut turns, mut shifts) = (Vec::new(), Vec::new(), Vec::new());
        for z in 0..k {
            for y in 0..k {
                for x in 0..k {
                    let middle = (x, y, z) == (k / 2, k / 2, k / 2);
                    let turn = if middle {
                        Orientation::IDENTITY
                    } else {
                        orients[rng.below(0, 48) as usize]
                    };
                    rooms.push(cut.add_room("block", ivec3(s, s, s)));
                    turns.push(turn);
                    // Slide the turned block back onto 0..s.
                    shifts.push(-turn.apply(IVec3::ZERO).min(turn.apply(far)));
                }
            }
        }
        let mut blocks = Blocks {
            whole,
            whole_room,
            cut,
            k,
            s,
            rooms,
            turns,
            shifts,
        };
        for z in 0..k {
            for y in 0..k {
                for x in 0..k {
                    for axis in 0..3 {
                        let a = ivec3(x, y, z);
                        if a[axis] + 1 < k {
                            blocks.glue(a, axis);
                        }
                    }
                }
            }
        }
        for z in 0..k * s {
            for y in 0..k * s {
                for x in 0..k * s {
                    if rng.unit() < 0.1 {
                        let at = ivec3(x, y, z);
                        let material = rng.below(1, 200) as u8;
                        blocks.whole.room_mut(whole_room).set(at, material);
                        let (cell, _) = blocks.to_cut(at);
                        blocks.cut.room_mut(cell.room).set(cell.at, material);
                    }
                }
            }
        }
        blocks
    }

    fn index(&self, block: IVec3) -> usize {
        (block[0] + self.k * (block[1] + self.k * block[2])) as usize
    }

    /// A voxel of the whole cube: where it is in the cut one, and the turn
    /// of the block it is in.
    fn to_cut(&self, at: IVec3) -> (Cell, Orientation) {
        let s = self.s;
        let block = ivec3(at[0] / s, at[1] / s, at[2] / s);
        let i = self.index(block);
        let local = at - ivec3(block[0] * s, block[1] * s, block[2] * s);
        let cell = Cell {
            room: self.rooms[i],
            at: self.turns[i].apply(local) + self.shifts[i],
        };
        (cell, self.turns[i])
    }

    /// Glue block `a` to the next block along `axis`.
    fn glue(&mut self, a: IVec3, axis: usize) {
        let s = self.s;
        let mut b = a;
        b[axis] += 1;
        // The last layer of `a` and the first of `b`, in the whole cube.
        let mut a_lo = ivec3(a[0] * s, a[1] * s, a[2] * s);
        let a_hi = a_lo + ivec3(s - 1, s - 1, s - 1);
        a_lo[axis] = a_hi[axis];
        let b_lo = ivec3(b[0] * s, b[1] * s, b[2] * s);
        let mut b_hi = b_lo + ivec3(s - 1, s - 1, s - 1);
        b_hi[axis] = b_lo[axis];

        let rect = |blocks: &Blocks, lo: IVec3, hi: IVec3, out: Face| {
            let ((p, turn), (q, _)) = (blocks.to_cut(lo), blocks.to_cut(hi));
            FaceRect {
                room: p.room,
                min: p.at.min(q.at),
                max: p.at.max(q.at),
                face: turn.face(out),
            }
        };
        let from = rect(self, a_lo, a_hi, Face::new(axis, true));
        let to = rect(self, b_lo, b_hi, Face::new(axis, false));
        let (ta, tb) = (self.turns[self.index(a)], self.turns[self.index(b)]);
        self.cut
            .glue(from, to, ta.inverse().then(tb))
            .expect("neighbouring blocks fit");
    }
}

/// The claim at its full strength: at every step, not only at the hit. A
/// ray is walked through a cube and through the same cube cut into 125
/// blocks, each turned its own way, and after every face the two cursors
/// must be at the same distance to the bit, in the same voxel, with the same
/// entry face and material. A ray crosses up to a dozen seams, each with
/// another turn, so the rule for ties is held after many crossings and under
/// reflections. Directions with whole-number components make most steps ties
/// between axes whose `t_delta` differ.
#[test]
fn every_step_agrees_through_many_turned_blocks() {
    let (k, s) = (5, 3);
    let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15);
    let middle = ivec3(k / 2 * s, k / 2 * s, k / 2 * s);
    let (mut steps, mut deep, mut reflected) = (0u32, 0u32, 0u32);

    for wi in 0..4 {
        let blocks = Blocks::new(k, s, &mut rng);
        let start_room = blocks.to_cut(middle).0.room;

        let mut rays = Vec::new();
        // Random rays. Origins are on a grid fine enough to be exact in
        // both worlds, whose coordinates differ by a whole number.
        for _ in 0..400 {
            let place = |rng: &mut Rng| rng.below(0, s << 12) as f32 / 4096.0;
            let local = vec3(place(&mut rng), place(&mut rng), place(&mut rng));
            rays.push((local, rng.direction()));
        }
        // Lattice rays: from voxel centres and voxel corners, along every
        // direction with components from -2 to 2.
        for cell in 0..s * s * s {
            let at = ivec3(cell % s, cell / s % s, cell / (s * s));
            for d in 0..125 {
                let dir = vec3(
                    (d % 5) as f32 - 2.0,
                    (d / 5 % 5) as f32 - 2.0,
                    (d / 25) as f32 - 2.0,
                );
                if dir != vec3(0.0, 0.0, 0.0) {
                    rays.push((at.centre(), dir));
                    rays.push((at.corner(), dir));
                }
            }
        }

        for (ri, &(local, dir)) in rays.iter().enumerate() {
            let ctx = format!("world {wi}, ray {ri} from {local:?} along {dir:?}");
            let mut a = Cursor::new(
                &blocks.whole,
                Ray::new(blocks.whole_room, local + middle.corner(), dir),
            );
            let mut b = Cursor::new(&blocks.cut, Ray::new(start_room, local, dir));
            loop {
                assert_eq!(a.next_t().to_bits(), b.next_t().to_bits(), "{ctx}");
                match (a.advance(), b.advance()) {
                    (Moved::Left, Moved::Left) => break,
                    (Moved::Entered { through: fa, .. }, Moved::Entered { through: fb, .. }) => {
                        let (cell, turn) = blocks.to_cut(a.cell().at);
                        assert_eq!(b.cell(), cell, "{ctx}, step {steps}");
                        assert_eq!(a.t().to_bits(), b.t().to_bits(), "{ctx}");
                        assert_eq!(b.frame(), turn, "{ctx}");
                        assert_eq!(fb, turn.face(fa), "{ctx}");
                        assert_eq!(a.material(), b.material(), "{ctx}");
                        steps += 1;
                        deep += (b.crossings() >= 3) as u32;
                        reflected += turn.is_reflection() as u32;
                    }
                    (x, y) => panic!("{ctx}: {x:?} in the whole cube, {y:?} in the cut one"),
                }
            }
        }
    }
    assert!(steps > 200_000, "{steps}");
    assert!(deep > 60_000, "{deep}");
    assert!(reflected > 40_000, "{reflected}");
}

/// A ray with no direction, or with one too small to have a reciprocal,
/// reaches no face. It must go nowhere whether or not there is a seam beside
/// it, even with no limit on distance.
#[test]
fn a_ray_that_reaches_no_face_stays_put_with_or_without_a_seam() {
    let build = |wrapped: bool| {
        let mut w = World::new();
        let id = w.add_room("r", ivec3(4, 4, 4));
        w.room_mut(id).set(ivec3(1, 1, 3), 9);
        if wrapped {
            w.wrap(id, 2).unwrap();
        }
        (w, id)
    };
    let opts = TraceOpts {
        max_t: f32::INFINITY,
        max_steps: 5,
        ..Default::default()
    };
    for dir in [
        vec3(0.0, 0.0, 0.0),
        vec3(1e-39, 0.0, 0.0),
        vec3(0.0, 0.0, -0.0),
    ] {
        for wrapped in [false, true] {
            let (w, id) = build(wrapped);
            let ray = Ray::new(id, vec3(1.5, 1.5, 0.5), dir);
            let hit = trace(&w, ray, &opts);
            let ctx = format!("{dir:?}, wrapped: {wrapped}");
            assert_eq!(hit.kind, HitKind::Far, "{ctx}");
            assert_eq!((hit.steps, hit.crossings), (0, 0), "{ctx}");
            assert_eq!(hit.cell.at, ivec3(1, 1, 0), "{ctx}");
            assert_eq!(Cursor::new(&w, ray).advance(), Moved::Left, "{ctx}");
        }
    }
}
