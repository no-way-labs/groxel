//! The Tower: a walkway round a tower that goes down all the way round and
//! arrives where it began.
//!
//! East of the tower, space is cut along a half-plane and glued back sixty
//! voxels out of step: a screw dislocation. Nothing is hidden from any
//! angle; every part of the tower is where it looks like it is. It is only
//! the whole that cannot exist.

use crate::demo::{Demo, Spot};
use crate::palette::{lin, norm};
use crate::{gallery, mat};
use groxel::{Face, FaceRect, Orientation, RoomId, SeamError, ivec3, vec3};
use groxel_render::Env;

/// The room is this many voxels along x and along z.
const SIZE: i32 = 120;
/// The room's height.
const HEIGHT: i32 = 400;
/// The tower's footprint runs from here ...
const T0: i32 = 46;
/// ... to here, inclusive, along x and along z.
const T1: i32 = 73;
/// The walkway's width.
const WALK: i32 = 6;
/// The parapet ring is at this coordinate ...
const W0: i32 = T0 - WALK - 1;
/// ... and at this one.
const W1: i32 = T1 + WALK + 1;
/// The tread height just north of the cut.
pub const TOP: i32 = 230;
/// The descent in one lap.
pub const DROP: i32 = 60;
/// The cut lies between z = `CUT - 1` and z = `CUT`.
const CUT: i32 = 60;

/// The tread height for a column outside the tower, or `None` inside it.
/// It is defined all the way out to the edge of the room, so everything can
/// be built relative to it.
pub fn lap(x: i32, z: i32) -> Option<i32> {
    let (e, w) = (x > T1, x < T0);
    let (n, s) = (z > T1, z < T0);
    if !e && !w && !n && !s {
        return None;
    }
    // The four landings, then the four flights between them.
    if e && n {
        return Some(TOP - 7);
    }
    if w && n {
        return Some(TOP - 22);
    }
    if w && s {
        return Some(TOP - 37);
    }
    if e && s {
        return Some(TOP - 52);
    }
    if e {
        return Some(if z >= CUT {
            TOP - (z - CUT).div_euclid(2)
        } else {
            TOP - 53 - (z - T0).div_euclid(2)
        });
    }
    if n {
        return Some(TOP - 8 - (T1 - x).div_euclid(2));
    }
    if w {
        return Some(TOP - 23 - (T1 - z).div_euclid(2));
    }
    Some(TOP - 38 - (x - T0).div_euclid(2))
}

/// Where feet go at a column.
pub fn feet(x: i32, z: i32) -> f32 {
    (lap(x, z).unwrap_or(TOP) + 1) as f32
}

// The lanes are squares about the middle of the room, which is at 60. One
// file of walkers goes down by the parapet, 19 out from the middle, and one
// comes up by the tower, 15.4 out.

/// Between the two files of walkers, 17.2 out from the middle of the room:
/// where a visitor can walk freely. The far side of that square is at 42.8.
pub const MID: f32 = 77.2;
/// The way down, by the parapet: the corners of one lap as (x, z).
pub const DOWN_LANE: [[f32; 2]; 4] = [[79.0, 79.0], [41.0, 79.0], [41.0, 41.0], [79.0, 41.0]];
/// The way up, by the tower: the corners of one lap as (x, z).
pub const UP_LANE: [[f32; 2]; 4] = [[75.4, 75.4], [75.4, 44.6], [44.6, 44.6], [44.6, 75.4]];

pub(crate) fn build(d: &mut Demo, gallery: RoomId) -> Result<(), SeamError> {
    let env = Env {
        zenith: lin([0.52, 0.66, 0.86]),
        horizon: lin([0.90, 0.86, 0.80]),
        nadir: lin([0.84, 0.82, 0.80]),
        haze: lin([0.88, 0.85, 0.80]),
        sun_dir: norm([0.62, 0.50, -0.60]),
        sun: [1.75, 1.38, 0.95],
        sky_light: [0.30, 0.40, 0.60],
        ground_light: [0.26, 0.23, 0.22],
        lamp: [0.50, 0.30, 0.12],
        fog: 0.0062,
        ..Default::default()
    };
    let id = d.add_room("tower", ivec3(SIZE, HEIGHT, SIZE), env, true);
    let r = d.world.room_mut(id);

    for z in 0..SIZE {
        for x in 0..SIZE {
            let Some(h) = lap(x, z) else {
                r.fill(ivec3(x, 0, z), ivec3(x, HEIGHT - 1, z), mat::LIMESTONE);
                continue;
            };
            // Beyond the parapet there is only air.
            if !(W0..=W1).contains(&x) || !(W0..=W1).contains(&z) {
                continue;
            }
            r.fill(ivec3(x, h - 5, z), ivec3(x, h, z), mat::LIMESTONE);
            if x == W0 || x == W1 || z == W0 || z == W1 {
                r.fill(ivec3(x, h - 7, z), ivec3(x, h + 3, z), mat::MARBLE);
                if (x + z).rem_euclid(4) < 2 {
                    r.set(ivec3(x, h + 4, z), mat::MARBLE);
                }
            }
        }
    }

    // Dress the four faces: string courses and lit windows that step down
    // with the stairs beside them. Each face is a voxel of the tower's skin,
    // the step inward from it, and the column outside it.
    for a in T0..=T1 {
        let faces = [
            ([T1, a], [-1, 0], [T1 + 1, a]),
            ([T0, a], [1, 0], [T0 - 1, a]),
            ([a, T1], [0, -1], [a, T1 + 1]),
            ([a, T0], [0, 1], [a, T0 - 1]),
        ];
        // Two voxels in every six along a face hold a window.
        let glazed = (2..=3).contains(&a.rem_euclid(6));
        for (cell, inward, outside) in faces {
            let h = lap(outside[0], outside[1]).expect("a column outside the tower");
            for y in 0..HEIGHT {
                let rel = (y - h).rem_euclid(24);
                if rel == 12 || rel == 13 {
                    r.set(ivec3(cell[0], y, cell[1]), mat::SLATE);
                }
                if (16..=21).contains(&rel) && glazed {
                    r.set(ivec3(cell[0], y, cell[1]), 0);
                    r.set(
                        ivec3(cell[0] + inward[0], y, cell[1] + inward[1]),
                        mat::WINDOW,
                    );
                }
            }
        }
    }

    // The red gate on the west side: the thing you keep coming back to.
    {
        let gz = T0 + 1;
        let h = lap(W0, gz).expect("the parapet is outside the tower");
        r.fill(ivec3(W0, h + 4, gz), ivec3(W0, h + 12, gz), mat::LACQUER);
        r.fill(
            ivec3(W0 - 1, h + 12, gz),
            ivec3(T0 - 1, h + 13, gz),
            mat::LACQUER,
        );
        r.fill(
            ivec3(W0, h + 10, gz),
            ivec3(T0 - 1, h + 10, gz),
            mat::LACQUER,
        );
        r.fill(
            ivec3(W0 - 1, h + 14, gz),
            ivec3(T0 - 1, h + 14, gz),
            mat::SLATE,
        );
    }

    // Lanterns on the parapet at each corner.
    for [x, z] in [[W0, W0], [W1, W0], [W1, W1], [W0, W1]] {
        let h = lap(x, z).expect("the parapet is outside the tower");
        r.fill(ivec3(x, h + 4, z), ivec3(x, h + 6, z), mat::IRON);
        r.set(ivec3(x, h + 7, z), mat::LAMP);
        r.set(ivec3(x, h + 8, z), mat::IRON);
    }

    // The cut. Going north across it on the east side lifts you one lap.
    d.world.glue(
        FaceRect {
            room: id,
            min: ivec3(T1 + 1, 0, CUT - 1),
            max: ivec3(SIZE - 1, HEIGHT - 1 - DROP, CUT - 1),
            face: Face::PosZ,
        },
        FaceRect {
            room: id,
            min: ivec3(T1 + 1, DROP, CUT),
            max: ivec3(SIZE - 1, HEIGHT - 1, CUT),
            face: Face::NegZ,
        },
        Orientation::IDENTITY,
    )?;

    // A door standing in the south parapet of the south-east landing.
    {
        let h = TOP - 52;
        let dz = W0;
        let r = d.world.room_mut(id);
        r.fill(ivec3(74, h + 1, dz), ivec3(79, h + 10, dz), mat::GOLD);
        let door = FaceRect {
            room: id,
            min: ivec3(75, h + 1, dz),
            max: ivec3(78, h + 8, dz),
            face: Face::NegZ,
        };
        r.fill(door.min, door.max, 0);
        d.world
            .glue_up(gallery::arch(gallery, 1), door, Face::PosY, Face::PosY)?;
    }

    d.spots.push(Spot {
        name: "tower",
        room: id,
        pos: vec3(MID, (TOP - 52 + 1) as f32, 43.0),
        yaw: 0.2,
        pitch: -0.42,
        frame: Orientation::IDENTITY,
        fly: false,
    });
    d.spots.push(Spot {
        name: "overlook",
        room: id,
        pos: vec3(106.0, (TOP - 14) as f32, 12.0),
        yaw: -0.72,
        pitch: -0.30,
        frame: Orientation::IDENTITY,
        fly: true,
    });
    d.add_walkers(id, &DOWN_LANE, 9, feet, 4.6);
    d.add_walkers(id, &UP_LANE, 7, feet, 4.2);
    Ok(())
}
