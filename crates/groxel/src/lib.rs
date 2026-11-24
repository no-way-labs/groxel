//! Groxel: voxels whose neighbours are whatever you say they are.
//!
//! In an ordinary voxel world the cell east of `(x, y, z)` is `(x + 1, y, z)`.
//! Here that is only the default. A [`Seam`] makes a rectangle of voxel faces
//! lead somewhere else: to another room, or back into the same one, turned
//! any of the 48 ways a cube can be turned. Everything that moves, moves by
//! crossing faces, so rays, poses and your own queries all follow seams
//! without knowing they are there.
//!
//! ```
//! use groxel::{ivec3, vec3, HitKind, Ray, TraceOpts, World, trace};
//!
//! let mut world = World::new();
//!
//! // A corridor with a floor. Its far end opens onto its near end.
//! let corridor = world.add_room("corridor", ivec3(32, 12, 8));
//! world.room_mut(corridor).fill(ivec3(0, 0, 0), ivec3(31, 0, 7), 1);
//! world.wrap(corridor, 0)?;
//!
//! // A ray fired down it comes round and hits the post behind you.
//! world.room_mut(corridor).fill(ivec3(2, 1, 4), ivec3(2, 8, 4), 2);
//! let ray = Ray::new(corridor, vec3(10.5, 3.5, 4.5), vec3(1.0, 0.0, 0.0));
//! let hit = trace(&world, ray, &TraceOpts::default());
//!
//! assert_eq!(hit.kind, HitKind::Solid);
//! assert_eq!(hit.material, 2);
//! assert_eq!(hit.crossings, 1);
//! assert!((hit.t - 23.5).abs() < 1e-4);
//! # Ok::<(), groxel::SeamError>(())
//! ```
//!
//! The crate is the core only: rooms, seams, poses and the tracer. It draws
//! nothing and has no dependencies.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod orient;
mod overlay;
mod pose;
mod trace;
mod vec;
mod world;

pub use orient::{Face, Orientation};
pub use overlay::{EntityId, Overlay, Scene, Skip, Space, Sprite, Stamp};
pub use pose::{Pose, Stopped, Travel};
pub use trace::{Ahead, Cursor, Hit, HitKind, Moved, Ray, TraceOpts, trace, visit};
pub use vec::{IVec3, Vec3, ivec3, vec3};
pub use world::{
    Cell, FaceRect, LatticeMap, Room, RoomId, Seam, SeamError, SeamId, SeamKind, World,
};
