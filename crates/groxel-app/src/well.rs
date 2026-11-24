//! The Well: a square stairwell whose top opens onto its own bottom.
//!
//! Thirty-two voxels of descent bring you back to the step you left.

use crate::demo::{Demo, Spot};
use crate::palette::{lin, norm};
use crate::{gallery, mat};
use groxel::{Face, FaceRect, Orientation, RoomId, SeamError, ivec3, vec3};
use groxel_render::Env;

/// The room is this many voxels along x and along z.
pub const SIZE: i32 = 42;
/// The room's height, which is the descent of one lap of the stairs.
pub const PERIOD: i32 = 32;
/// The first voxel inside the wall.
const LO: i32 = 2;
/// The last voxel inside the wall.
const HI: i32 = SIZE - 3;
/// The stairs' width, including the rail.
const BAND: i32 = 7;
/// The first voxel of a straight run.
const A: i32 = LO + BAND;
/// The last voxel of a straight run.
const B: i32 = HI - BAND;
/// The tread height of the south-west landing.
pub const TOP: i32 = 27;

/// The height of the tread at a column, or `None` over the open shaft and
/// inside the walls. Heights run on below zero: the room wraps, so take
/// them modulo [`PERIOD`] to find the voxel.
pub fn tread(x: i32, z: i32) -> Option<i32> {
    if in_wall(x, z) {
        return None;
    }
    let (south, north) = (z < A, z > B);
    let (west, east) = (x < A, x > B);
    // The four landings, then the four runs between them.
    if south && west {
        return Some(TOP);
    }
    if south && east {
        return Some(TOP - 8);
    }
    if north && east {
        return Some(TOP - 16);
    }
    if north && west {
        return Some(TOP - 24);
    }
    if south {
        return Some(TOP - 1 - (x - A).div_euclid(3));
    }
    if east {
        return Some(TOP - 9 - (z - A).div_euclid(3));
    }
    if north {
        return Some(TOP - 17 - (B - x).div_euclid(3));
    }
    if west {
        return Some(TOP - 25 - (B - z).div_euclid(3));
    }
    None
}

/// True for a column of the wall round the well.
fn in_wall(x: i32, z: i32) -> bool {
    !(LO..=HI).contains(&x) || !(LO..=HI).contains(&z)
}

/// Where feet go at a column, kept low enough that the body's middle is
/// inside the room. The rest of the body is found through the wrap.
pub fn feet(x: i32, z: i32) -> f32 {
    let mut y = tread(x, z).unwrap_or(0).rem_euclid(PERIOD) + 1;
    // A body's middle is 3.3 above its feet, and walkers start 1.25 high.
    if y + 5 > PERIOD {
        y -= PERIOD;
    }
    y as f32
}

/// The way down, by the wall: the corners of one lap as (x, z).
pub const DOWN_LANE: [[f32; 2]; 4] = [[4.0, 4.0], [38.0, 4.0], [38.0, 38.0], [4.0, 38.0]];
/// The way up, by the rail: the corners of one lap as (x, z).
pub const UP_LANE: [[f32; 2]; 4] = [[6.4, 35.6], [35.6, 35.6], [35.6, 6.4], [6.4, 6.4]];

pub(crate) fn build(d: &mut Demo, gallery: RoomId) -> Result<(), SeamError> {
    let env = Env {
        zenith: lin([0.42, 0.34, 0.26]),
        horizon: lin([0.80, 0.88, 0.97]),
        nadir: lin([0.04, 0.05, 0.09]),
        haze: lin([0.07, 0.06, 0.08]),
        sun_dir: norm([0.30, 0.90, 0.34]),
        sun: [0.0, 0.0, 0.0],
        sky_light: [0.50, 0.66, 0.95],
        ground_light: [0.34, 0.44, 0.62],
        lamp: [2.1, 1.15, 0.50],
        floor: 0.03,
        fog: 0.0065,
    };
    let id = d.add_room("well", ivec3(SIZE, PERIOD, SIZE), env, false);
    let r = d.world.room_mut(id);
    let wrapped = |y: i32| y.rem_euclid(PERIOD);

    for z in 0..SIZE {
        for x in 0..SIZE {
            let Some(h) = tread(x, z) else {
                if in_wall(x, z) {
                    r.fill(ivec3(x, 0, z), ivec3(x, PERIOD - 1, z), mat::BRICK);
                }
                continue;
            };
            r.set(ivec3(x, wrapped(h), z), mat::LIMESTONE);
            r.set(ivec3(x, wrapped(h - 1), z), mat::LIMESTONE);

            // Balustrade: the ring of columns that border the open shaft.
            let ring = A - 1..=B + 1;
            let facing_shaft = ring.contains(&x) && ring.contains(&z);
            if facing_shaft {
                r.set(ivec3(x, wrapped(h + 3), z), mat::SLATE);
                if (x + z).rem_euclid(2) == 0 {
                    r.set(ivec3(x, wrapped(h + 1), z), mat::MARBLE);
                    r.set(ivec3(x, wrapped(h + 2), z), mat::MARBLE);
                }
            }
        }
    }

    // A tall window in the middle of each wall, following the stairs down.
    // Each is a box through the wall, and the column whose tread it follows.
    let mid = SIZE / 2;
    let walls = [
        ([mid - 2, mid + 1], [0, 1], [mid, LO]),
        ([SIZE - 2, SIZE - 1], [mid - 2, mid + 1], [HI, mid]),
        ([mid - 2, mid + 1], [SIZE - 2, SIZE - 1], [mid, HI]),
        ([0, 1], [mid - 2, mid + 1], [LO, mid]),
    ];
    for (xs, zs, column) in walls {
        let h = tread(column[0], column[1]).expect("a stair runs along each wall");
        for y in h + 4..=h + 13 {
            let yy = wrapped(y);
            r.fill(ivec3(xs[0], yy, zs[0]), ivec3(xs[1], yy, zs[1]), 0);
        }
    }

    // A lamp chain down the middle of the shaft: one lamp, seen forever.
    r.fill(ivec3(mid, 0, mid), ivec3(mid, PERIOD - 1, mid), mat::IRON);
    r.fill(
        ivec3(mid - 1, 13, mid - 1),
        ivec3(mid + 1, 15, mid + 1),
        mat::LAMP,
    );
    r.fill(
        ivec3(mid - 1, 16, mid - 1),
        ivec3(mid + 1, 16, mid + 1),
        mat::IRON,
    );
    r.fill(
        ivec3(mid - 1, 12, mid - 1),
        ivec3(mid + 1, 12, mid + 1),
        mat::IRON,
    );

    // A wall lamp in the outer corner of each landing.
    for [x, z] in [[LO, LO], [HI, LO], [HI, HI], [LO, HI]] {
        let h = tread(x, z).expect("a landing fills each corner");
        r.set(ivec3(x, wrapped(h + 8), z), mat::IRON);
        r.set(ivec3(x, wrapped(h + 9), z), mat::LAMP);
        r.set(ivec3(x, wrapped(h + 10), z), mat::IRON);
    }

    // One red newel post on the south-west landing: the proof that it is one
    // landing, not many. The room is only PERIOD high, so the post stops at
    // the top of it and the gold cap, which would be above that, is clipped
    // away.
    let post_y = TOP + 1;
    r.fill(
        ivec3(A - 1, post_y, A - 1),
        ivec3(A - 1, post_y + 4, A - 1),
        mat::LACQUER,
    );
    r.set(ivec3(A - 1, post_y + 5, A - 1), mat::GOLD);

    d.world.wrap(id, 1)?;

    // A door in the east wall of the north-east landing, back to the gallery.
    let door_y = TOP - 16 + 1;
    let door = FaceRect {
        room: id,
        min: ivec3(SIZE - 2, door_y, 35),
        max: ivec3(SIZE - 2, door_y + 7, 38),
        face: Face::PosX,
    };
    d.world.room_mut(id).fill(door.min, door.max, 0);
    d.world
        .glue_up(gallery::arch(gallery, 0), door, Face::PosY, Face::PosY)?;

    d.spots.push(Spot {
        name: "well",
        room: id,
        pos: vec3(5.5, (TOP + 1) as f32, 5.0),
        yaw: (std::f64::consts::PI * 0.30) as f32,
        pitch: -0.25,
        frame: Orientation::IDENTITY,
        fly: false,
    });
    // One file of walkers climbing by the rail; the wall side is left clear
    // for whoever comes down.
    d.add_walkers(id, &UP_LANE, 7, feet, 4.2);
    Ok(())
}
