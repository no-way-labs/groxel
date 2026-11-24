//! The walking body. The central test is the tracer's: a world cut in two,
//! with one half turned any of the 48 ways and glued back, is walked exactly
//! as the uncut world is, and "up" turns with the half you are in.

use groxel::{Face, Orientation, Ray, RoomId, TraceOpts, World, ivec3, trace, vec3};
use groxel_body::{Body, Input};
use std::f32::consts::FRAC_PI_2;

#[path = "../../groxel/tests/common/mod.rs"]
mod common;
use common::CutWorld;

const DT: f32 = 1.0 / 60.0;

fn floor_room(w: &mut World, name: &str, dims: [i32; 3]) -> RoomId {
    let id = w.add_room(name, ivec3(dims[0], dims[1], dims[2]));
    w.room_mut(id)
        .fill(ivec3(0, 0, 0), ivec3(dims[0] - 1, 0, dims[2] - 1), 1);
    id
}

fn settle(b: &mut Body, w: &World) {
    for _ in 0..120 {
        b.update(w, &Input::default(), DT);
    }
}

#[test]
fn falls_lands_stands() {
    let mut w = World::new();
    let id = floor_room(&mut w, "r", [16, 16, 16]);
    let mut b = Body::new(id, vec3(8.0, 5.0, 8.0));
    settle(&mut b, &w);
    assert!(b.on_ground);
    assert!((b.pos[1] - 1.0).abs() < 1e-4);
}

#[test]
fn walls_stop_it_and_steps_do_not() {
    let mut w = World::new();
    let id = floor_room(&mut w, "r", [32, 16, 16]);
    w.room_mut(id).fill(ivec3(12, 1, 0), ivec3(13, 1, 15), 1); // a one-voxel step
    w.room_mut(id).fill(ivec3(20, 1, 0), ivec3(21, 9, 15), 1); // a wall
    let mut b = Body::new(id, vec3(5.0, 1.0, 8.0));
    b.yaw = FRAC_PI_2;
    settle(&mut b, &w);
    let forward = Input {
        forward: 1.0,
        ..Default::default()
    };
    for _ in 0..600 {
        b.update(&w, &forward, DT);
    }
    // Over the step, down the other side, and up against the wall.
    assert!((b.pos[0] - (20.0 - 0.9)).abs() < 0.02, "{:?}", b.pos);
    assert!((b.pos[1] - 1.0).abs() < 1e-3);
}

#[test]
fn round_a_wrapped_corridor_and_back_where_it_started() {
    let mut w = World::new();
    let id = floor_room(&mut w, "ring", [24, 12, 8]);
    w.wrap(id, 0).unwrap();
    let mut b = Body::new(id, vec3(3.0, 1.0, 4.0));
    b.yaw = FRAC_PI_2;
    settle(&mut b, &w);
    let forward = Input {
        forward: 1.0,
        ..Default::default()
    };
    let mut travelled = 0.0;
    for _ in 0..60 * 12 {
        b.update(&w, &forward, DT);
        travelled += b.walk_speed * DT;
    }
    assert_eq!(b.crossings, 5);
    let expect = (3.0 + travelled).rem_euclid(24.0);
    assert!((b.pos[0] - expect).abs() < 0.05, "{} vs {expect}", b.pos[0]);
    assert!(b.on_ground);
}

#[test]
fn a_mirror_stops_a_body() {
    let mut w = World::new();
    let id = floor_room(&mut w, "m", [16, 12, 8]);
    w.mirror(groxel::FaceRect {
        room: id,
        min: ivec3(15, 0, 0),
        max: ivec3(15, 11, 7),
        face: Face::PosX,
    })
    .unwrap();
    let mut b = Body::new(id, vec3(8.0, 1.0, 4.0));
    b.yaw = FRAC_PI_2;
    settle(&mut b, &w);
    let forward = Input {
        forward: 1.0,
        ..Default::default()
    };
    for _ in 0..300 {
        b.update(&w, &forward, DT);
    }
    assert_eq!(b.crossings, 0);
    // It rests against the glass, half its width from it, as against a wall.
    assert!((b.pos[0] - (16.0 - 0.9)).abs() < 0.02, "{:?}", b.pos);
}

/// Cut a floored world in two, turn the far half each of the 48 ways and glue
/// it back. A body that walks across, climbs a step, goes round a post and
/// turns must end where it ends in the uncut world, standing on the floor of
/// the turned half with its head pointing where that half's "up" now is.
#[test]
fn walking_through_a_turned_seam_matches_the_uncut_world_all_48_ways() {
    let n = 12;
    for (ti, turn) in Orientation::all().enumerate() {
        let mut cw = CutWorld::new(n, turn);
        for z in 0..n {
            for x in 0..2 * n {
                cw.set(ivec3(x, 0, z), 1); // the floor
            }
            for x in n + 3..=n + 5 {
                cw.set(ivec3(x, 1, z), 1); // a step to climb
            }
        }
        for y in 1..=9 {
            cw.set(ivec3(n + 8, y, 5), 1); // a post to go round
        }

        let start = vec3(4.0, 1.0, 4.5);
        let mut want = Body::new(cw.whole_room, start);
        let mut got = Body::new(cw.left, start);
        want.yaw = FRAC_PI_2;
        got.yaw = FRAC_PI_2;
        for i in 0..60 * 3 {
            let input = Input {
                forward: 1.0,
                strafe: if i > 60 && i < 100 { 0.5 } else { 0.0 },
                turn: if i > 120 { 0.01 } else { 0.0 },
                ..Default::default()
            };
            want.update(&cw.whole, &input, DT);
            got.update(&cw.cut, &input, DT);
        }
        let ctx = format!("turn #{ti} {turn:?}");

        // It got well into the far half.
        assert!(want.pos[0] > (n + 6) as f32, "{ctx}: {:?}", want.pos);
        assert_eq!(got.room, cw.right, "{ctx}");
        assert_eq!(got.crossings, 1, "{ctx}");
        // Its frame is the turn, so its "up" is where the turn put +y.
        assert_eq!(got.frame, turn, "{ctx}");
        assert_eq!(got.up_face(), turn.face(Face::PosY), "{ctx}");
        assert_eq!(want.on_ground, got.on_ground, "{ctx}");
        assert!(got.on_ground, "{ctx}");

        let expect = cw.point_in_right(want.pos);
        for k in 0..3 {
            assert!(
                (expect[k] - got.pos[k]).abs() < 2e-3,
                "{ctx}: {expect:?} vs {:?}",
                got.pos
            );
        }

        // And the two eyes see the same thing.
        let (vw, vg) = (want.view(&cw.whole), got.view(&cw.cut));
        let opts = TraceOpts::default();
        let ray = |v: groxel_body::View| Ray {
            start: v.cell,
            origin: v.pos,
            dir: v.fwd,
        };
        let (hw, hg) = (
            trace(&cw.whole, ray(vw), &opts),
            trace(&cw.cut, ray(vg), &opts),
        );
        assert_eq!(hw.kind, hg.kind, "{ctx}");
        assert!((hw.t - hg.t).abs() < 5e-3, "{ctx}: {} vs {}", hw.t, hg.t);
    }
}

/// A mirror is glass: a body that falls onto one stands on it, and can walk
/// off it. Before this was tested, the body's feet sank through the mirror
/// into its own reflection and it came to rest inside the wall behind,
/// unable to move.
#[test]
fn a_body_lands_on_a_mirror_and_walks_off_it() {
    let mut w = World::new();
    let id = w.add_room("m", ivec3(24, 16, 12));
    // A solid floor, and a mirror over the western half of it.
    w.room_mut(id).fill(ivec3(0, 0, 0), ivec3(23, 0, 11), 1);
    w.mirror(groxel::FaceRect {
        room: id,
        min: ivec3(0, 1, 0),
        max: ivec3(11, 1, 11),
        face: Face::NegY,
    })
    .unwrap();

    let mut b = Body::new(id, vec3(5.0, 8.0, 6.0));
    b.yaw = FRAC_PI_2;
    settle(&mut b, &w);
    // It stands on the glass, not in it.
    assert!(b.on_ground);
    assert!((b.pos[1] - 1.0).abs() < 1e-3, "{:?}", b.pos);

    let forward = Input {
        forward: 1.0,
        ..Default::default()
    };
    for _ in 0..90 {
        b.update(&w, &forward, DT);
    }
    // It has walked east, off the mirror and onto the plain floor.
    assert!(b.pos[0] > 14.0, "{:?}", b.pos);
    assert!((b.pos[1] - 1.0).abs() < 1e-3, "{:?}", b.pos);
    assert_eq!(b.crossings, 0);
}

/// Standing a body upright turns it about its middle, keeps it looking the
/// way it was looking, and eases the eye from the old view to the new one.
#[test]
fn standing_up_keeps_the_middle_and_the_look_and_eases_the_eye() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(32, 32, 32));
    w.room_mut(id).fill(ivec3(0, 0, 0), ivec3(31, 0, 31), 1);

    // Lying on its side in mid air: its head points along +z.
    let mut b = Body::new(id, vec3(16.0, 16.0, 12.0));
    b.frame = Orientation::quarter_turn(0, 1);
    assert_eq!(b.up_face(), Face::PosZ);
    b.yaw = 0.7;
    b.pitch = 0.2;
    let before = b.view(&w);
    let middle = b.pos + vec3(0.0, 0.0, b.height * 0.5);

    assert!(b.stand_up(&w, Face::PosY));
    assert_eq!(b.up_face(), Face::PosY);
    assert!(!b.frame.is_reflection());
    let now_middle = b.pos + vec3(0.0, b.height * 0.5, 0.0);
    assert!((now_middle - middle).length() < 1e-4);

    // At the instant of turning the eye has not moved and the picture is
    // the same picture.
    let at_once = b.view(&w);
    assert!(
        (at_once.fwd - before.fwd).length() < 1e-4,
        "{:?}",
        at_once.fwd
    );
    assert!((at_once.up - before.up).length() < 1e-3, "{:?}", at_once.up);
    assert!(
        (at_once.pos - before.pos).length() < 1e-3,
        "{:?}",
        at_once.pos
    );

    // Half a second on it is looking the same way with its head up, and
    // the eye is over the feet.
    let still = Input::default();
    let mut last_up = at_once.up;
    for _ in 0..30 {
        b.update(&w, &still, DT);
        let v = b.view(&w);
        // The view rolls a little at a time; it never jumps.
        assert!(
            (v.up - last_up).length() < 0.2,
            "{:?} after {last_up:?}",
            v.up
        );
        assert!((v.fwd - before.fwd).length() < 1e-4);
        last_up = v.up;
    }
    assert_eq!(b.tilt, 0.0);
    let after = b.view(&w);
    // No roll is left: the view's right is level, and its up has a rising
    // part. (How steeply it looks down has not changed.)
    assert!(after.right[1].abs() < 1e-4, "{:?}", after.right);
    assert!(after.up[1] > 0.0, "{:?}", after.up);
    assert!((after.pos[0] - b.pos[0]).abs() < 1e-4 && (after.pos[2] - b.pos[2]).abs() < 1e-4);

    // Upright already: nothing to do, and nothing changes.
    let copy = b;
    assert!(b.stand_up(&w, Face::PosY));
    assert_eq!(b, copy);

    // Then it falls to the floor and stands on it.
    settle(&mut b, &w);
    assert!(b.on_ground);
    assert!((b.pos[1] - 1.0).abs() < 1e-3);
}

/// A body lying in a tunnel too low to stand in is left as it is, and stands
/// up once it is out.
#[test]
fn a_body_stands_up_only_where_there_is_room() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(40, 16, 9));
    // Solid, with a tunnel 5 high along x that opens into a tall chamber.
    w.room_mut(id).fill(ivec3(0, 0, 0), ivec3(39, 15, 8), 1);
    w.room_mut(id).fill(ivec3(1, 1, 2), ivec3(24, 5, 6), 0);
    w.room_mut(id).fill(ivec3(25, 1, 2), ivec3(38, 14, 6), 0);

    // Lying along the tunnel, head towards +x, "standing" on the tunnel's
    // end wall as far as its own gravity is concerned.
    let mut b = Body::new(id, vec3(2.0, 3.5, 4.5));
    b.frame = Orientation::quarter_turn(2, 3);
    assert_eq!(b.up_face(), Face::PosX);
    let lying = b;
    assert!(!b.stand_up(&w, Face::PosY));
    assert_eq!(b, lying);

    // Fly it along its own "up" into the chamber and it can stand.
    b.fly = true;
    let rise = Input {
        rise: 1.0,
        ..Default::default()
    };
    let mut stood = false;
    for _ in 0..600 {
        b.update(&w, &rise, DT);
        if b.stand_up(&w, Face::PosY) {
            stood = true;
            break;
        }
    }
    assert!(stood, "{:?}", b.pos);
    assert_eq!(b.up_face(), Face::PosY);
    assert!(b.pos[0] > 24.0, "{:?}", b.pos);
}
