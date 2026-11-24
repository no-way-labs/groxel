//! The Hall: a square corridor whose far end opens onto its near end a
//! quarter turn round.
//!
//! Walk its length four times and you have stood on every wall.

use crate::demo::{Demo, Spot, Walker};
use crate::palette::lin;
use crate::{gallery, mat};
use groxel::{Face, FaceRect, Orientation, RoomId, SeamError, ivec3, vec3};
use groxel_body::Body;
use groxel_render::Env;

/// The corridor is this many voxels wide and this many high.
pub const SIDE: i32 = 22;
/// The corridor's length.
pub const LEN: i32 = 56;

pub(crate) fn build(d: &mut Demo, gallery: RoomId) -> Result<(), SeamError> {
    let env = Env {
        zenith: lin([0.55, 0.70, 0.92]),
        horizon: lin([0.84, 0.90, 0.98]),
        nadir: lin([0.70, 0.76, 0.84]),
        haze: lin([0.08, 0.07, 0.10]),
        sun: [0.0, 0.0, 0.0],
        sky_light: [0.62, 0.78, 1.05],
        ground_light: [0.42, 0.52, 0.72],
        lamp: [2.0, 1.15, 0.52],
        floor: 0.03,
        fog: 0.0045,
        ..Default::default()
    };
    let id = d.add_room("hall", ivec3(SIDE, SIDE, LEN), env, false);
    // Every wall of the hall is a floor to someone.
    d.turns_freely(id);
    let r = d.world.room_mut(id);
    let (s, l) = (SIDE - 1, LEN - 1);
    r.fill(ivec3(0, 0, 0), ivec3(s, 0, l), mat::CHECKER); // floor
    r.fill(ivec3(s, 0, 0), ivec3(s, s, l), mat::WOOD); // +x wall
    r.fill(ivec3(0, s, 0), ivec3(s, s, l), mat::PLASTER); // ceiling
    r.fill(ivec3(0, 0, 0), ivec3(0, s, l), mat::BRICK); // -x wall
    r.fill(ivec3(9, 0, 0), ivec3(12, 0, l), mat::CARPET);

    // Things that belong to the floor, so you can tell which wall it is now.
    for z in [14, 42] {
        r.fill(ivec3(4, 1, z), ivec3(4, 5, z), mat::IRON);
        r.set(ivec3(4, 6, z), mat::LAMP);
        r.set(ivec3(4, 7, z), mat::IRON);
    }
    r.fill(ivec3(2, 1, 28), ivec3(3, 2, 35), mat::WOOD);
    r.fill(ivec3(1, 1, 28), ivec3(1, 4, 35), mat::WOOD);
    // A window in the +x wall.
    r.fill(ivec3(s, 8, 26), ivec3(s, 14, 30), 0);

    d.world.glue(
        FaceRect {
            room: id,
            min: ivec3(0, 0, l),
            max: ivec3(s, s, l),
            face: Face::PosZ,
        },
        FaceRect {
            room: id,
            min: ivec3(0, 0, 0),
            max: ivec3(s, s, 0),
            face: Face::NegZ,
        },
        Orientation::quarter_turn(2, 1),
    )?;

    // Door to the gallery in the -x wall.
    let door = FaceRect {
        room: id,
        min: ivec3(0, 1, 18),
        max: ivec3(0, 8, 21),
        face: Face::NegX,
    };
    d.world.room_mut(id).fill(door.min, door.max, 0);
    d.world
        .glue_up(gallery::arch(gallery, 2), door, Face::PosY, Face::PosY)?;

    d.spots.push(Spot {
        name: "hall",
        room: id,
        pos: vec3(10.5, 1.0, 5.0),
        yaw: 0.0,
        pitch: -0.05,
        frame: Orientation::IDENTITY,
        fly: false,
    });

    // One figure on each surface, all walking the same way.
    let centre = 11.0;
    for k in 0..4 {
        let frame = Orientation::quarter_turn(2, k);
        let off = frame.apply_f(vec3(-5.0, -10.0, 0.0));
        let mut body = Body::new(
            id,
            vec3(centre + off[0], centre + off[1], 8.0 + 12.0 * k as f32),
        );
        body.frame = frame;
        body.walk_speed = 4.4;
        // Stand a hair off the surface and let gravity seat it.
        let up = body.up_face();
        body.pos[up.axis()] += up.sign() as f32 * 0.25;
        d.walkers.push(Walker {
            body,
            path: &[],
            next: 0,
            odometer: 0.0,
        });
    }
    Ok(())
}
