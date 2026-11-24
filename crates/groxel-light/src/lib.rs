//! Light that floods through a groxel world.
//!
//! Two levels are kept per voxel: lamp light, which starts at glowing voxels,
//! and sky light, which starts wherever empty space meets the open edge of a
//! room. Each spreads outward one voxel at a time and loses one level per
//! step. It spreads the way everything in a groxel world moves, across faces
//! and through seams, so a lamp lights the room on the far side of a doorway,
//! and in a room glued to itself one lamp lights every copy.
//!
//! ```
//! use groxel::{Cell, World, ivec3};
//! use groxel_light::{Light, REACH};
//!
//! let mut world = World::new();
//! let ring = world.add_room("ring", ivec3(8, 3, 3));
//! world.wrap(ring, 0)?;
//! world.room_mut(ring).set(ivec3(0, 1, 1), 9);
//!
//! let light = Light::flood(&world, |material| material == 9, |_| false);
//!
//! // The voxel on either side of the lamp is fully lit: one of them through
//! // the seam.
//! let at = |x| Cell { room: ring, at: ivec3(x, 1, 1) };
//! assert_eq!(light.lamp(at(1)), REACH);
//! assert_eq!(light.lamp(at(7)), REACH);
//! assert_eq!(light.lamp(at(4)), REACH - 3);
//! # Ok::<(), groxel::SeamError>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use groxel::{Cell, Face, IVec3, RoomId, World, ivec3};

/// The level a light starts with, which is also how many voxels it reaches.
pub const REACH: u8 = 40;

/// The lamp and sky light of every voxel of a world, as it was when
/// [`Light::flood`] ran.
#[derive(Clone, Debug)]
pub struct Light {
    rooms: Vec<RoomLight>,
}

#[derive(Clone, Debug)]
struct RoomLight {
    dims: IVec3,
    lamp: Vec<u8>,
    sky: Vec<u8>,
}

impl RoomLight {
    fn index(&self, c: IVec3) -> Option<usize> {
        let d = self.dims;
        let inside = (0..3).all(|i| c[i] >= 0 && c[i] < d[i]);
        inside.then(|| (c[0] + d[0] * (c[1] + d[1] * c[2])) as usize)
    }
}

#[derive(Copy, Clone)]
enum Field {
    Lamp,
    Sky,
}

impl Light {
    /// Work out both kinds of light for the whole world.
    ///
    /// `glows` says which materials are lamps. `outdoors` says which rooms
    /// are open air: every voxel of such a room has full sky light.
    pub fn flood(
        world: &World,
        glows: impl Fn(u8) -> bool,
        outdoors: impl Fn(RoomId) -> bool,
    ) -> Light {
        let ids = || (0..world.rooms().len() as u32).map(RoomId);
        let mut light = Light {
            rooms: ids()
                .map(|id| {
                    let dims = world.room(id).dims();
                    let n = (dims[0] * dims[1] * dims[2]) as usize;
                    RoomLight {
                        dims,
                        lamp: vec![0; n],
                        sky: vec![if outdoors(id) { REACH } else { 0 }; n],
                    }
                })
                .collect(),
        };
        let mut queue = Vec::new();

        // Lamp light starts in every empty voxel that touches a glowing one.
        for id in ids() {
            for at in cells(world.room(id).dims()) {
                if !glows(world.room(id).material(at)) {
                    continue;
                }
                for face in Face::ALL {
                    let Some((next, _)) = world.neighbour(Cell { room: id, at }, face) else {
                        continue;
                    };
                    light.seed(world, next, Field::Lamp, &mut queue);
                }
            }
        }
        light.spread(world, &mut queue, Field::Lamp);

        // Sky light starts in every empty voxel on a face of its room that
        // leads nowhere.
        queue.clear();
        for id in ids().filter(|&id| !outdoors(id)) {
            let dims = world.room(id).dims();
            for at in cells(dims) {
                let cell = Cell { room: id, at };
                let open = Face::ALL.into_iter().any(|face| {
                    let beyond = at + face.normal();
                    let edge = (0..3).any(|i| beyond[i] < 0 || beyond[i] >= dims[i]);
                    edge && world.seam_at(cell, face).is_none()
                });
                if open {
                    light.seed(world, cell, Field::Sky, &mut queue);
                }
            }
        }
        light.spread(world, &mut queue, Field::Sky);
        light
    }

    /// Lamp light at a voxel, from 0 to [`REACH`]. Zero outside the world.
    pub fn lamp(&self, cell: Cell) -> u8 {
        self.level(cell, Field::Lamp)
    }

    /// Sky light at a voxel, from 0 to [`REACH`]. Zero outside the world.
    pub fn sky(&self, cell: Cell) -> u8 {
        self.level(cell, Field::Sky)
    }

    fn level(&self, cell: Cell, field: Field) -> u8 {
        let Some(room) = self.rooms.get(cell.room.0 as usize) else {
            return 0;
        };
        match (room.index(cell.at), field) {
            (Some(i), Field::Lamp) => room.lamp[i],
            (Some(i), Field::Sky) => room.sky[i],
            (None, _) => 0,
        }
    }

    fn slot(&mut self, cell: Cell, field: Field) -> &mut u8 {
        let room = &mut self.rooms[cell.room.0 as usize];
        let i = room.index(cell.at).expect("a voxel of the world");
        match field {
            Field::Lamp => &mut room.lamp[i],
            Field::Sky => &mut room.sky[i],
        }
    }

    /// Give an empty voxel full light and queue it.
    fn seed(&mut self, world: &World, cell: Cell, field: Field, queue: &mut Vec<(Cell, u8)>) {
        if world.room(cell.room).is_solid(cell.at) {
            return;
        }
        let slot = self.slot(cell, field);
        if *slot != REACH {
            *slot = REACH;
            queue.push((cell, REACH));
        }
    }

    fn spread(&mut self, world: &World, queue: &mut Vec<(Cell, u8)>, field: Field) {
        // Every seed starts at the same level, so first in, first out is
        // breadth first.
        let mut head = 0;
        while head < queue.len() {
            let (cell, level) = queue[head];
            head += 1;
            if level <= 1 {
                continue;
            }
            for face in Face::ALL {
                let Some((next, _)) = world.neighbour(cell, face) else {
                    continue;
                };
                if world.room(next.room).is_solid(next.at) {
                    continue;
                }
                let slot = self.slot(next, field);
                if *slot < level - 1 {
                    *slot = level - 1;
                    queue.push((next, level - 1));
                }
            }
        }
    }
}

fn cells(dims: IVec3) -> impl Iterator<Item = IVec3> {
    (0..dims[2]).flat_map(move |z| {
        (0..dims[1]).flat_map(move |y| (0..dims[0]).map(move |x| ivec3(x, y, z)))
    })
}
