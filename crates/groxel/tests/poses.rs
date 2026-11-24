//! Poses: a point with an orientation, moved through seams.

use groxel::{Face, FaceRect, Orientation, Pose, Stopped, World, ivec3, vec3};

mod common;
use common::{CutWorld, Rng, hall};

#[test]
fn a_pose_crossing_a_turned_seam_matches_the_uncut_world_all_48_ways() {
    let n = 8;
    let mut rng = Rng::new(0xD1B5_4A32_D192_ED03);
    let mut moves_across = 0;

    for turn in Orientation::all() {
        let cw = CutWorld::new(n, turn);
        let start = vec3(2.3, 3.7, 4.1);
        let mut want = Pose::new(cw.whole_room, start);
        let mut got = Pose::new(cw.left, start);

        for step in 0..60 {
            // Head for the other half each time, so nearly every move crosses
            // the seam, with some wandering sideways. `delta` is in the whole
            // world's frame, which is also the body's own frame in the cut
            // world: say it in body axes there.
            let toward = if want.pos[0] < n as f32 { 1.0 } else { -1.0 };
            let delta = vec3(
                toward * (2.0 + 5.0 * rng.unit()),
                3.0 * rng.unit() - 1.5,
                3.0 * rng.unit() - 1.5,
            );
            let a = want.advance(&cw.whole, delta);
            let b = got.advance_local(&cw.cut, delta);
            let ctx = format!("turn {turn:?}, step {step}");

            assert_eq!(a.stopped, b.stopped.map(|_| Stopped::Edge), "{ctx}");
            assert!((a.distance - b.distance).abs() < 1e-3, "{ctx}");
            let (room, at) = cw.cell_in_cut(want.cell.at);
            assert_eq!((got.cell.room, got.cell.at), (room, at), "{ctx}");
            let on_right = want.cell.at[0] >= n;
            let expect_pos = if on_right {
                cw.point_in_right(want.pos)
            } else {
                want.pos
            };
            for i in 0..3 {
                assert!(
                    (expect_pos[i] - got.pos[i]).abs() < 2e-3,
                    "{ctx}: {expect_pos:?} vs {:?}",
                    got.pos
                );
            }
            assert_eq!(
                got.frame,
                if on_right {
                    turn
                } else {
                    Orientation::IDENTITY
                },
                "{ctx}"
            );
            moves_across += b.crossings;
        }
    }
    assert!(moves_across > 1500, "{moves_across}");
}

#[test]
fn round_a_wrapped_room_and_back_where_it_started() {
    let mut w = World::new();
    let r = w.add_room("ring", ivec3(24, 12, 8));
    w.wrap(r, 0).unwrap();
    let mut p = Pose::new(r, vec3(3.25, 5.5, 4.5));
    let mut crossings = 0;
    for _ in 0..96 {
        crossings += p.advance(&w, vec3(1.25, 0.0, 0.0)).crossings;
    }
    // 96 moves of 1.25 is five laps of 24.
    assert_eq!(crossings, 5);
    assert!((p.pos[0] - 3.25).abs() < 1e-3, "{:?}", p.pos);
    assert_eq!(p.frame, Orientation::IDENTITY);
}

#[test]
fn edges_and_mirrors_stop_a_pose_where_they_are() {
    let mut w = World::new();
    let r = w.add_room("r", ivec3(8, 4, 4));
    let glass = w
        .mirror(FaceRect {
            room: r,
            min: ivec3(7, 0, 0),
            max: ivec3(7, 3, 3),
            face: Face::PosX,
        })
        .unwrap();

    let mut p = Pose::new(r, vec3(5.5, 1.5, 1.5));
    let t = p.advance(&w, vec3(10.0, 0.0, 0.0));
    assert_eq!(t.stopped, Some(Stopped::Barrier(glass)));
    assert!((t.distance - 2.5).abs() < 1e-5);
    assert_eq!(p.pos, vec3(8.0, 1.5, 1.5));
    assert_eq!(p.cell.at, ivec3(7, 1, 1));
    assert_eq!(p.frame, Orientation::IDENTITY);

    let t = p.advance(&w, vec3(-20.0, 0.0, 0.0));
    assert_eq!(t.stopped, Some(Stopped::Edge));
    assert_eq!(p.pos, vec3(0.0, 1.5, 1.5));
    assert_eq!(p.cell.at, ivec3(0, 1, 1));

    // And it can still move away from where it stopped.
    let t = p.advance(&w, vec3(1.5, 0.0, 0.0));
    assert_eq!(t.stopped, None);
    assert_eq!(p.pos, vec3(1.5, 1.5, 1.5));
}

#[test]
fn four_lengths_of_the_hall_stand_you_on_every_wall_and_bring_you_home() {
    let mut w = World::new();
    let id = hall::build(&mut w).unwrap();
    let start = vec3(10.5, 4.5, 5.25);
    let mut p = Pose::new(id, start);
    let forward = vec3(0.0, 0.0, hall::LEN as f32);

    let mut ups = Vec::new();
    for _ in 0..4 {
        ups.push(p.up());
        let t = p.advance_local(&w, forward);
        assert_eq!((t.crossings, t.stopped), (1, None));
    }
    assert_eq!(ups, [Face::PosY, Face::NegX, Face::NegY, Face::PosX]);
    assert_eq!(p.frame, Orientation::IDENTITY);
    for i in 0..3 {
        assert!((p.pos[i] - start[i]).abs() < 1e-3, "{:?}", p.pos);
    }

    // The carpet is underfoot at the start. One length on, the same walker,
    // still feeling upright, has the wooden wall underfoot.
    let below = |p: &Pose, w: &World| {
        let (mut cell, mut found) = (p.cell, 0);
        let down = p.frame.face(Face::NegY);
        for _ in 0..hall::SIDE {
            match w.neighbour(cell, down) {
                Some((next, _)) => cell = next,
                None => break,
            }
            found = w.room(cell.room).material(cell.at);
            if found != 0 {
                break;
            }
        }
        found
    };
    assert_eq!(below(&p, &w), hall::CARPET);
    p.advance_local(&w, forward);
    assert_eq!(below(&p, &w), hall::WOOD);
    p.advance_local(&w, forward);
    assert_eq!(below(&p, &w), hall::PLASTER);
    p.advance_local(&w, forward);
    assert_eq!(below(&p, &w), hall::BRICK);
}
