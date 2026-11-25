//! Voxels that are in the world for one frame.
//!
//! Things that move do not edit the world. Each frame their voxels are put
//! into an [`Overlay`], and rays are traced against the world and the overlay
//! together (a [`Scene`]). The world is not changed, so it can be read from
//! every thread while the next overlay is being made.
//!
//! A sprite is placed by walking the lattice from its anchor. A figure
//! standing in a doorway therefore has its front half in the next room, the
//! right way round, with nothing special done for it.

use crate::orient::Orientation;
use crate::vec::{IVec3, ivec3};
use crate::world::{Cell, RoomId, World};
use std::collections::HashMap;

/// Names a moving thing. A trace can be told to look past one entity's
/// voxels: see [`Skip`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntityId(pub u32);

/// A small block of materials: a figure, a prop.
///
/// It is modelled standing on y = 0 and facing +z. `origin` is the voxel
/// that sits on the anchor it is placed at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    dims: IVec3,
    origin: IVec3,
    vox: Vec<u8>,
}

impl Sprite {
    /// A sprite from its voxels, `vox[x + dims.x * (y + dims.y * z)]`, with 0
    /// for empty.
    ///
    /// # Panics
    /// If `vox` is not `dims.x * dims.y * dims.z` long.
    pub fn new(dims: IVec3, origin: IVec3, vox: Vec<u8>) -> Sprite {
        assert_eq!(
            vox.len(),
            (dims[0] * dims[1] * dims[2]) as usize,
            "a sprite needs one material per voxel"
        );
        Sprite { dims, origin, vox }
    }

    /// A sprite from layers of text, bottom layer first. Each layer lists its
    /// rows from the back (z = 0) to the front. `key` maps characters to
    /// materials; any other character is empty. The origin is the middle of
    /// the bottom layer.
    ///
    /// ```
    /// use groxel::{Sprite, ivec3};
    ///
    /// // A post two voxels tall with a cap.
    /// let post = Sprite::from_layers(&[&["#"], &["#"], &["o"]], &[('#', 4), ('o', 9)]);
    /// assert_eq!(post.dims(), ivec3(1, 3, 1));
    /// assert_eq!(post.material(ivec3(0, 2, 0)), 9);
    /// ```
    ///
    /// # Panics
    /// If the layers or their rows differ in size.
    pub fn from_layers(layers: &[&[&str]], key: &[(char, u8)]) -> Sprite {
        let h = layers.len();
        let d = layers.first().map_or(0, |layer| layer.len());
        let w = layers
            .first()
            .and_then(|layer| layer.first())
            .map_or(0, |row| row.chars().count());
        let mut vox = vec![0; w * h * d];
        for (y, layer) in layers.iter().enumerate() {
            assert_eq!(layer.len(), d, "every layer needs the same number of rows");
            for (z, row) in layer.iter().enumerate() {
                assert_eq!(row.chars().count(), w, "every row needs the same length");
                for (x, ch) in row.chars().enumerate() {
                    if let Some(&(_, m)) = key.iter().find(|(k, _)| *k == ch) {
                        vox[x + w * (y + h * z)] = m;
                    }
                }
            }
        }
        Sprite {
            dims: ivec3(w as i32, h as i32, d as i32),
            origin: ivec3(w as i32 / 2, 0, d as i32 / 2),
            vox,
        }
    }

    /// Size in voxels.
    pub fn dims(&self) -> IVec3 {
        self.dims
    }

    /// The voxel that sits on the anchor.
    pub fn origin(&self) -> IVec3 {
        self.origin
    }

    /// The material at a voxel of the sprite. Outside it is empty.
    pub fn material(&self, c: IVec3) -> u8 {
        let d = self.dims;
        if (0..3).all(|i| c[i] >= 0 && c[i] < d[i]) {
            self.vox[(c[0] + d[0] * (c[1] + d[1] * c[2])) as usize]
        } else {
            0
        }
    }
}

/// One voxel of an overlay.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// What it is made of. Never 0.
    pub material: u8,
    /// Whose it is.
    pub entity: EntityId,
}

#[derive(Clone, Debug)]
struct RoomOverlay {
    dims: IVec3,
    /// One bit per voxel of the room, so the tracer's test is a load and a
    /// mask wherever nothing is stamped.
    occupied: Vec<u64>,
    stamps: HashMap<u32, Stamp>,
}

/// A sparse set of voxels laid over a world for one frame.
#[derive(Clone, Debug, Default)]
pub struct Overlay {
    rooms: Vec<RoomOverlay>,
}

impl Overlay {
    /// An empty overlay for this world. It must be made again if rooms are
    /// added to the world.
    pub fn new(world: &World) -> Overlay {
        Overlay {
            rooms: world
                .rooms()
                .iter()
                .map(|room| {
                    let d = room.dims();
                    let voxels = (d[0] * d[1] * d[2]) as usize;
                    RoomOverlay {
                        dims: d,
                        occupied: vec![0; voxels.div_ceil(64)],
                        stamps: HashMap::new(),
                    }
                })
                .collect(),
        }
    }

    /// Take every voxel out.
    pub fn clear(&mut self) {
        for room in &mut self.rooms {
            for &i in room.stamps.keys() {
                room.occupied[i as usize / 64] = 0;
            }
            room.stamps.clear();
        }
    }

    /// How many voxels are in it.
    pub fn len(&self) -> usize {
        self.rooms.iter().map(|room| room.stamps.len()).sum()
    }

    /// True if nothing is in it.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Place `sprite` with its origin at `at`. `orient` takes the sprite's
    /// axes to the axes of `at`'s room. Each voxel is reached by walking the
    /// lattice from `at`, so the sprite follows any seam it straddles.
    ///
    /// A voxel is left out if the world is solid there, if something is
    /// already stamped there, or if the walk leaves the world or meets a
    /// mirror: a figure does not reach through glass.
    pub fn put(
        &mut self,
        world: &World,
        at: Cell,
        orient: Orientation,
        sprite: &Sprite,
        entity: EntityId,
    ) {
        let d = sprite.dims;
        for z in 0..d[2] {
            for y in 0..d[1] {
                for x in 0..d[0] {
                    let c = ivec3(x, y, z);
                    let material = sprite.material(c);
                    if material == 0 {
                        continue;
                    }
                    let Some((there, _)) = world.walk_open(at, orient.apply(c - sprite.origin))
                    else {
                        continue;
                    };
                    let room = world.room(there.room);
                    if room.is_solid(there.at) {
                        continue;
                    }
                    let i = room.index(there.at);
                    let over = &mut self.rooms[there.room.0 as usize];
                    over.stamps.entry(i as u32).or_insert_with(|| {
                        over.occupied[i / 64] |= 1 << (i % 64);
                        Stamp { material, entity }
                    });
                }
            }
        }
    }

    /// What is stamped at a voxel, if anything.
    pub fn get(&self, cell: Cell) -> Option<Stamp> {
        let over = self.rooms.get(cell.room.0 as usize)?;
        let (c, d) = (cell.at, over.dims);
        if (0..3).any(|i| c[i] < 0 || c[i] >= d[i]) {
            return None;
        }
        self.at(cell.room, (c[0] + d[0] * (c[1] + d[1] * c[2])) as usize)
    }

    /// Every stamped voxel and what is stamped there, in no particular
    /// order. Cheaper than [`get`](Overlay::get) at every voxel of the world
    /// when a few figures are stamped into large rooms.
    pub fn iter(&self) -> impl Iterator<Item = (Cell, Stamp)> + '_ {
        self.rooms.iter().enumerate().flat_map(|(r, over)| {
            let d = over.dims;
            over.stamps.iter().map(move |(&i, &stamp)| {
                let i = i as i32;
                let at = ivec3(i % d[0], i / d[0] % d[1], i / (d[0] * d[1]));
                let room = RoomId(r as u32);
                (Cell { room, at }, stamp)
            })
        })
    }

    /// What is stamped at a voxel given by its index in its room.
    pub(crate) fn at(&self, room: RoomId, index: usize) -> Option<Stamp> {
        let over = &self.rooms[room.0 as usize];
        if over.occupied[index / 64] & (1 << (index % 64)) == 0 {
            return None;
        }
        over.stamps.get(&(index as u32)).copied()
    }
}

/// Something rays can be traced through: a [`World`], or a world with an
/// overlay on it.
pub trait Space {
    /// The world.
    fn world(&self) -> &World;
    /// The frame's moving voxels, if there are any.
    fn overlay(&self) -> Option<&Overlay>;
}

impl Space for World {
    fn world(&self) -> &World {
        self
    }

    fn overlay(&self) -> Option<&Overlay> {
        None
    }
}

/// A world with an overlay on it.
#[derive(Copy, Clone, Debug)]
pub struct Scene<'a> {
    /// The world.
    pub world: &'a World,
    /// This frame's moving voxels.
    pub overlay: &'a Overlay,
}

impl Space for Scene<'_> {
    fn world(&self) -> &World {
        self.world
    }

    fn overlay(&self) -> Option<&Overlay> {
        Some(self.overlay)
    }
}

/// Tells a trace to look past one entity's voxels near the start of the ray.
///
/// A viewer's own figure is in the overlay like any other. Seen directly it
/// would fill the view, so its voxels are skipped until the ray has gone at
/// least `near`. Beyond that they are solid: you see your own back down a
/// corridor that loops.
///
/// `near` should be no less than the reach of the figure from the eye. The
/// rule reads the distance only. It does not ask whether the ray has crossed
/// a seam, because the answer would change when a world is cut and glued
/// back. An image of the viewer that is nearer than `near`, in a mirror the
/// viewer is standing against, is therefore not drawn either.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Skip {
    /// Whose voxels to look past.
    pub entity: EntityId,
    /// How far the ray must have gone before they count.
    pub near: f32,
}
