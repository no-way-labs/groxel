//! Small helpers for building rooms by hand.
//!
//! Figures are built from text by [`groxel::Sprite::from_layers`], which is
//! in the core.

use groxel::{Face, IVec3, Room, ivec3};

/// A solid box, corners inclusive.
pub fn solid_box(room: &mut Room, lo: IVec3, hi: IVec3, material: u8) {
    room.fill(lo, hi, material);
}

/// An empty box, corners inclusive.
pub fn carve(room: &mut Room, lo: IVec3, hi: IVec3) {
    room.fill(lo, hi, 0);
}

/// A hollow box with walls one voxel thick.
pub fn shell(room: &mut Room, lo: IVec3, hi: IVec3, material: u8) {
    room.fill(lo, hi, material);
    room.fill(lo + ivec3(1, 1, 1), hi - ivec3(1, 1, 1), 0);
}

/// A flight of stairs for [`stairs`] to build.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Stairs {
    /// The lowest voxel of the first tread, at the corner the flight starts
    /// from.
    pub at: IVec3,
    /// The horizontal direction of travel going up.
    pub dir: Face,
    /// Voxels across, counted along the other horizontal axis from `at`.
    pub width: i32,
    /// How many treads.
    pub steps: i32,
    /// Voxels forward per tread.
    pub run: i32,
    /// Voxels up per tread.
    pub rise: i32,
    /// How far below each tread the flight is solid.
    pub thick: i32,
    /// What it is made of.
    pub material: u8,
}

impl Stairs {
    /// A flight with treads two voxels deep, one voxel high and two voxels
    /// thick.
    pub fn new(at: IVec3, dir: Face, width: i32, steps: i32, material: u8) -> Stairs {
        Stairs {
            at,
            dir,
            width,
            steps,
            run: 2,
            rise: 1,
            thick: 2,
            material,
        }
    }
}

/// Build a flight of stairs.
pub fn stairs(room: &mut Room, s: &Stairs) {
    let along = s.dir.axis();
    let across = if along == 0 { 2 } else { 0 };
    for i in 0..s.steps {
        let mut lo = s.at;
        lo[along] += s.dir.sign() * i * s.run;
        lo[1] += i * s.rise;
        let mut hi = lo;
        hi[along] += s.dir.sign() * (s.run - 1);
        hi[across] += s.width - 1;
        lo[1] -= s.thick - 1;
        room.fill(lo, hi, s.material);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use groxel::World;

    #[test]
    fn stairs_climb() {
        let mut w = World::new();
        let id = w.add_room("r", ivec3(16, 16, 8));
        let r = w.room_mut(id);
        stairs(r, &Stairs::new(ivec3(2, 3, 1), Face::PosX, 3, 4, 5));
        assert!(r.is_solid(ivec3(2, 3, 1)) && r.is_solid(ivec3(3, 3, 3)));
        assert!(r.is_solid(ivec3(8, 6, 2)) && r.is_solid(ivec3(8, 5, 2)));
        assert!(!r.is_solid(ivec3(8, 4, 2)) && !r.is_solid(ivec3(2, 4, 1)));
    }

    #[test]
    fn a_shell_is_hollow() {
        let mut w = World::new();
        let id = w.add_room("r", ivec3(8, 8, 8));
        let r = w.room_mut(id);
        shell(r, ivec3(1, 1, 1), ivec3(5, 4, 6), 3);
        assert_eq!(r.material(ivec3(1, 2, 3)), 3);
        assert_eq!(r.material(ivec3(5, 4, 6)), 3);
        assert!(!r.is_solid(ivec3(2, 2, 2)) && !r.is_solid(ivec3(4, 3, 5)));
        assert!(!r.is_solid(ivec3(0, 2, 3)) && !r.is_solid(ivec3(6, 2, 3)));

        solid_box(r, ivec3(2, 2, 2), ivec3(4, 3, 5), 7);
        assert_eq!(r.material(ivec3(3, 3, 4)), 7);
        carve(r, ivec3(3, 2, 2), ivec3(3, 3, 5));
        assert!(!r.is_solid(ivec3(3, 3, 4)));
        assert_eq!(r.material(ivec3(2, 3, 4)), 7);
    }
}
