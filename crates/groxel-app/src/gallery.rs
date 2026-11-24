//! The Gallery: an ordinary room with three doors and a mirror.
//!
//! The doors open onto the other worlds. The mirror is a seam from each
//! voxel of glass to itself.

use crate::demo::{Demo, Spot};
use crate::palette::lin;
use crate::{kit, mat};
use groxel::{Face, FaceRect, Orientation, RoomId, SeamError, ivec3, vec3};
use groxel_render::Env;

const W: i32 = 46;
const H: i32 = 18;
const DEPTH: i32 = 32;

/// The x of the middle of each door: to the well, the tower and the hall.
pub const CENTRES: [i32; 3] = [11, 23, 35];

/// The opening of door `i` of the gallery `room`, as the faces you walk out
/// through.
pub fn arch(room: RoomId, i: usize) -> FaceRect {
    let c = CENTRES[i];
    FaceRect {
        room,
        min: ivec3(c - 2, 1, DEPTH - 1),
        max: ivec3(c + 1, 8, DEPTH - 1),
        face: Face::PosZ,
    }
}

pub(crate) fn build(d: &mut Demo) -> Result<RoomId, SeamError> {
    let env = Env {
        haze: lin([0.10, 0.09, 0.09]),
        sun: [0.0, 0.0, 0.0],
        sky_light: [0.5, 0.6, 0.8],
        ground_light: [0.3, 0.36, 0.5],
        lamp: [1.9, 1.35, 0.80],
        floor: 0.04,
        fog: 0.004,
        ..Default::default()
    };
    let id = d.add_room("gallery", ivec3(W, H, DEPTH), env, false);
    let r = d.world.room_mut(id);
    let (x1, y1, z1) = (W - 1, H - 1, DEPTH - 1);
    kit::shell(r, ivec3(0, 0, 0), ivec3(x1, y1, z1), mat::PLASTER);
    r.fill(ivec3(0, 0, 0), ivec3(x1, 0, z1), mat::CHECKER);
    r.fill(ivec3(1, 1, z1), ivec3(x1 - 1, 2, z1), mat::WOOD); // skirting on the door wall
    // Ceiling lamps.
    for lx in [6, 14, 22, 30, 38] {
        for lz in [7, 15, 23] {
            r.fill(ivec3(lx, y1, lz), ivec3(lx + 1, y1, lz + 1), mat::LAMP);
        }
    }
    // Three doors, each with its own colour of frame.
    let frames = [mat::LACQUER, mat::GOLD, mat::AZURE];
    for (c, m) in CENTRES.into_iter().zip(frames) {
        r.fill(ivec3(c - 3, 1, z1 - 1), ivec3(c + 2, 10, z1 - 1), m);
        r.fill(ivec3(c - 2, 1, z1 - 1), ivec3(c + 1, 8, z1), 0);
    }
    // A plinth in the middle.
    r.fill(ivec3(21, 1, 12), ivec3(24, 2, 15), mat::MARBLE);
    r.fill(ivec3(22, 3, 13), ivec3(23, 4, 14), mat::GOLD);
    // The mirror, along the wall facing the doors.
    r.fill(ivec3(7, 1, 0), ivec3(38, 13, 0), mat::SLATE);
    d.world.mirror(FaceRect {
        room: id,
        min: ivec3(8, 1, 1),
        max: ivec3(37, 12, 1),
        face: Face::NegZ,
    })?;

    d.spots.push(Spot {
        name: "gallery",
        room: id,
        pos: vec3(23.0, 1.0, 7.0),
        yaw: 0.0,
        pitch: 0.0,
        frame: Orientation::IDENTITY,
        fly: false,
    });
    Ok(id)
}
