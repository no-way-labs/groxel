//! Shared by the integration tests and the examples. Uses the public API only.
#![allow(dead_code)]

use groxel::{Face, FaceRect, IVec3, Orientation, RoomId, SeamError, Vec3, World, ivec3, vec3};

/// A small deterministic generator, so the tests need no dependencies.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform in 0..1.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in lo..hi.
    pub fn below(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u64) as i32
    }

    /// A unit vector, uniform over directions that are not nearly zero.
    pub fn direction(&mut self) -> Vec3 {
        loop {
            let v = vec3(
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
                self.unit() * 2.0 - 1.0,
            );
            let len = v.length();
            if (0.2..=1.0).contains(&len) {
                return v * (1.0 / len);
            }
        }
    }
}

/// A world of `2n x n x n` cut in half along x, with the right half turned by
/// `turn` and glued back. `to_cut` converts a voxel or point of the whole
/// world to where it is in the cut one.
pub struct CutWorld {
    pub whole: World,
    pub whole_room: RoomId,
    pub cut: World,
    pub left: RoomId,
    pub right: RoomId,
    pub n: i32,
    pub turn: Orientation,
    shift: IVec3,
}

impl CutWorld {
    pub fn new(n: i32, turn: Orientation) -> CutWorld {
        let mut whole = World::new();
        let whole_room = whole.add_room("whole", ivec3(2 * n, n, n));
        let mut cut = World::new();
        let left = cut.add_room("left", ivec3(n, n, n));
        let right = cut.add_room("right", ivec3(n, n, n));

        // Slide the turned cube back onto 0..n.
        let far = ivec3(n - 1, n - 1, n - 1);
        let shift = -turn.apply(IVec3::ZERO).min(turn.apply(far));

        // The left half's +x face meets whichever layer of the right half
        // the turn puts x = 0 on.
        let j = turn.axis_to(0);
        let mut lo = IVec3::ZERO;
        let mut hi = far;
        lo[j] = shift[j];
        hi[j] = shift[j];
        cut.glue(
            FaceRect {
                room: left,
                min: ivec3(n - 1, 0, 0),
                max: far,
                face: Face::PosX,
            },
            FaceRect {
                room: right,
                min: lo,
                max: hi,
                face: turn.face(Face::PosX).opposite(),
            },
            turn,
        )
        .expect("the halves fit");
        CutWorld {
            whole,
            whole_room,
            cut,
            left,
            right,
            n,
            turn,
            shift,
        }
    }

    /// Put the same material at the same place in both worlds.
    pub fn set(&mut self, at: IVec3, material: u8) {
        self.whole.room_mut(self.whole_room).set(at, material);
        let (room, there) = self.cell_in_cut(at);
        self.cut.room_mut(room).set(there, material);
    }

    pub fn cell_in_cut(&self, at: IVec3) -> (RoomId, IVec3) {
        if at[0] < self.n {
            (self.left, at)
        } else {
            (
                self.right,
                self.turn.apply(at - ivec3(self.n, 0, 0)) + self.shift,
            )
        }
    }

    /// A point of the whole world's right half, in the right room's frame.
    pub fn point_in_right(&self, p: Vec3) -> Vec3 {
        let half = vec3(0.5, 0.5, 0.5);
        let local = p - vec3(self.n as f32, 0.0, 0.0);
        self.turn.apply_f(local - half)
            + half
            + vec3(
                self.shift[0] as f32,
                self.shift[1] as f32,
                self.shift[2] as f32,
            )
    }
}

/// The Hall: a square corridor whose far end opens onto its near end a
/// quarter turn round.
pub mod hall {
    use super::*;

    pub const SIDE: i32 = 22;
    pub const LEN: i32 = 56;

    pub const CHECKER: u8 = 1;
    pub const WOOD: u8 = 2;
    pub const PLASTER: u8 = 3;
    pub const BRICK: u8 = 4;
    pub const CARPET: u8 = 5;
    pub const IRON: u8 = 6;
    pub const LAMP: u8 = 7;

    pub fn build(world: &mut World) -> Result<RoomId, SeamError> {
        let id = world.add_room("hall", ivec3(SIDE, SIDE, LEN));
        let (s, l) = (SIDE - 1, LEN - 1);
        let room = world.room_mut(id);
        room.fill(ivec3(0, 0, 0), ivec3(s, 0, l), CHECKER); // floor
        room.fill(ivec3(s, 0, 0), ivec3(s, s, l), WOOD); // +x wall
        room.fill(ivec3(0, s, 0), ivec3(s, s, l), PLASTER); // ceiling
        room.fill(ivec3(0, 0, 0), ivec3(0, s, l), BRICK); // -x wall
        room.fill(ivec3(9, 0, 0), ivec3(12, 0, l), CARPET);
        for z in [14, 42] {
            room.fill(ivec3(4, 1, z), ivec3(4, 5, z), IRON);
            room.set(ivec3(4, 6, z), LAMP);
            room.set(ivec3(4, 7, z), IRON);
        }
        room.fill(ivec3(2, 1, 28), ivec3(3, 2, 35), WOOD); // a bench
        room.fill(ivec3(s, 8, 26), ivec3(s, 14, 30), 0); // a window

        world.glue(
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
        Ok(id)
    }
}
