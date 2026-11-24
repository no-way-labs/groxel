//! Where something is and which way round it is.

use crate::orient::{Face, Orientation};
use crate::trace::{Cursor, Ray};
use crate::vec::Vec3;
use crate::world::{Cell, RoomId, SeamId, SeamKind, World};

/// A place and an orientation: everything needed to say where a thing is.
///
/// A pose is small, `Copy`, and complete, so it is what a save file or a
/// network message carries. Its `frame` is how the thing's own axes read in
/// the room it is in. It changes when the pose crosses a turned seam, which
/// is how "down" comes to mean a different wall of the same room.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pose {
    /// The voxel the point is in. Kept alongside `pos` because a point on a
    /// boundary does not say which side it is on.
    pub cell: Cell,
    /// The point, in the room's frame. Always within `cell`.
    pub pos: Vec3,
    /// The thing's own axes to the room's axes.
    pub frame: Orientation,
}

/// Why a move ended early.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Stopped {
    /// At the edge of a room, where nothing leads on.
    Edge,
    /// At a seam that lets sight through but not things.
    Barrier(SeamId),
}

/// What happened during a move.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Travel {
    /// Seams crossed.
    pub crossings: u32,
    /// How far it got, in voxels.
    pub distance: f32,
    /// Set if it could not go the whole way.
    pub stopped: Option<Stopped>,
}

impl Pose {
    /// A pose at a point, unturned.
    pub fn new(room: RoomId, pos: Vec3) -> Pose {
        Pose {
            cell: Cell {
                room,
                at: pos.floor(),
            },
            pos,
            frame: Orientation::IDENTITY,
        }
    }

    /// The room it is in.
    pub fn room(&self) -> RoomId {
        self.cell.room
    }

    /// Which way the thing's own +y points in the room.
    pub fn up(&self) -> Face {
        self.frame.face(Face::PosY)
    }

    /// A direction in the thing's own axes, as it reads in the room.
    pub fn to_room(&self, local: Vec3) -> Vec3 {
        self.frame.apply_f(local)
    }

    /// Move in a straight line by `delta`, given in the room's frame,
    /// crossing any seams on the way. Solid voxels are not obstacles here:
    /// a pose is a point, and collision belongs to whatever owns it.
    pub fn advance(&mut self, world: &World, delta: Vec3) -> Travel {
        let len = delta.length();
        if len == 0.0 {
            return Travel {
                crossings: 0,
                distance: 0.0,
                stopped: None,
            };
        }
        let mut cur = Cursor::new(
            world,
            Ray {
                start: self.cell,
                origin: self.pos,
                dir: delta * (1.0 / len),
            },
        );
        let mut reach = len;
        let mut stopped = None;
        loop {
            let ahead = cur.ahead();
            if ahead.t > len {
                break;
            }
            let barrier = ahead
                .seam
                .filter(|&id| world.seam(id).kind == SeamKind::SightOnly);
            if ahead.leaves || barrier.is_some() {
                stopped = Some(match barrier {
                    Some(id) => Stopped::Barrier(id),
                    None => Stopped::Edge,
                });
                reach = ahead.t;
                break;
            }
            cur.advance();
        }
        self.cell = cur.cell();
        let mut pos = cur.origin() + cur.dir() * reach;
        // Rounding must not put the point outside the voxel the walk ended in.
        for i in 0..3 {
            let lo = self.cell.at[i] as f32;
            pos[i] = pos[i].clamp(lo, lo + 1.0);
        }
        self.pos = pos;
        self.frame = self.frame.then(cur.frame());
        Travel {
            crossings: cur.crossings(),
            distance: reach,
            stopped,
        }
    }

    /// Move by `delta` given in the thing's own axes.
    pub fn advance_local(&mut self, world: &World, delta: Vec3) -> Travel {
        self.advance(world, self.to_room(delta))
    }
}
