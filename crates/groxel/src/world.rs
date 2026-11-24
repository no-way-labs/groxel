//! Rooms, seams and the world that holds them.
//!
//! A room is a plain grid of voxels with its own coordinates. Inside it, the
//! neighbour of a voxel is the next cell over. A seam overrides that for a
//! rectangle of faces: step through one of them and you are in another place,
//! given by the seam's [`LatticeMap`].

use crate::orient::{Face, Orientation};
use crate::vec::{IVec3, Vec3, ivec3};
use std::fmt;

/// Names a room in a [`World`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoomId(pub u32);

/// Names a seam in a [`World`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SeamId(pub u32);

/// A voxel: which room, and where in it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    /// The room.
    pub room: RoomId,
    /// The voxel's coordinates in that room.
    pub at: IVec3,
}

/// A rigid map of the voxel lattice: turn, then shift.
///
/// A seam's map is a change of coordinates. It converts anything near the
/// seam (a voxel, a point, a direction) from the frame of the room on one
/// side into the frame of the room on the other. All of it is integer
/// arithmetic, so maps compose and invert without rounding.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LatticeMap {
    /// The turn.
    pub rot: Orientation,
    /// The shift applied after turning.
    pub offset: IVec3,
}

impl LatticeMap {
    /// Changes nothing.
    pub const IDENTITY: LatticeMap = LatticeMap {
        rot: Orientation::IDENTITY,
        offset: IVec3::ZERO,
    };

    /// A pure shift.
    pub const fn shift(offset: IVec3) -> LatticeMap {
        LatticeMap {
            rot: Orientation::IDENTITY,
            offset,
        }
    }

    /// Convert a voxel.
    pub fn cell(self, c: IVec3) -> IVec3 {
        self.rot.apply(c) + self.offset
    }

    /// Convert a point. A voxel's centre goes to the centre of its image.
    pub fn point(self, p: Vec3) -> Vec3 {
        let half = Vec3([0.5, 0.5, 0.5]);
        self.rot.apply_f(p - half) + half + self.offset.corner()
    }

    /// Convert a direction.
    pub fn dir(self, d: Vec3) -> Vec3 {
        self.rot.apply_f(d)
    }

    /// This map followed by `next`.
    pub fn then(self, next: LatticeMap) -> LatticeMap {
        LatticeMap {
            rot: self.rot.then(next.rot),
            offset: next.rot.apply(self.offset) + next.offset,
        }
    }

    /// The map that undoes this one.
    pub fn inverse(self) -> LatticeMap {
        let rot = self.rot.inverse();
        LatticeMap {
            rot,
            offset: -rot.apply(self.offset),
        }
    }
}

/// A box of voxels (corners inclusive) and one face of each.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FaceRect {
    /// The room the voxels are in.
    pub room: RoomId,
    /// Lowest corner.
    pub min: IVec3,
    /// Highest corner, inclusive.
    pub max: IVec3,
    /// Which face of each voxel.
    pub face: Face,
}

impl FaceRect {
    /// True if `c` is one of the rectangle's voxels.
    pub fn contains(&self, c: IVec3) -> bool {
        (0..3).all(|i| c[i] >= self.min[i] && c[i] <= self.max[i])
    }

    /// How many voxels along each axis.
    pub fn size(&self) -> IVec3 {
        self.max - self.min + ivec3(1, 1, 1)
    }
}

/// What may cross a seam.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SeamKind {
    /// Everything crosses: rays, light and bodies.
    Open,
    /// Rays and light cross; a [`Pose`](crate::Pose) is stopped. A mirror.
    SightOnly,
}

/// A rectangle of faces that all lead somewhere else, by one map.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Seam {
    /// The faces you leave through.
    pub from: FaceRect,
    /// The room you arrive in.
    pub to: RoomId,
    /// Coordinates in `from.room` to coordinates in `to`.
    pub map: LatticeMap,
    /// What may cross.
    pub kind: SeamKind,
}

/// Why a seam could not be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeamError {
    /// The rectangle's corners are the wrong way round.
    EmptyRect {
        /// The rectangle as given.
        rect: FaceRect,
    },
    /// Part of the rectangle is outside its room.
    OutsideRoom {
        /// The rectangle as given.
        rect: FaceRect,
        /// The room's size.
        dims: IVec3,
    },
    /// The seam would lead to voxels outside the destination room.
    LandsOutside {
        /// One voxel it would lead to.
        cell: Cell,
        /// The destination room's size.
        dims: IVec3,
    },
    /// Walking out of one side does not walk into the other.
    FacesDoNotMeet {
        /// The face you leave by.
        leave: Face,
        /// That face after the turn.
        turned: Face,
        /// The face of the far side you would have to come in against.
        needed: Face,
    },
    /// The two rectangles are different shapes once turned.
    ShapesDiffer {
        /// Size of the near rectangle, turned.
        turned: IVec3,
        /// Size of the far rectangle.
        far: IVec3,
    },
    /// A face in the rectangle already belongs to a seam.
    Overlap {
        /// The voxel.
        cell: Cell,
        /// Its face.
        face: Face,
        /// The seam already there.
        existing: SeamId,
    },
    /// `glue_up` was given an "up" that lies along the way through.
    UpAlongTheWay,
    /// A mirror's rectangle must be one voxel thick along the axis it faces.
    MirrorNotFlat {
        /// The rectangle as given.
        rect: FaceRect,
    },
}

impl fmt::Display for SeamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeamError::EmptyRect { rect } => {
                write!(
                    f,
                    "the rectangle {:?}..{:?} is empty: every min must be <= its max",
                    rect.min.0, rect.max.0
                )
            }
            SeamError::OutsideRoom { rect, dims } => write!(
                f,
                "the rectangle {:?}..{:?} does not fit in its room, which is {:?} voxels",
                rect.min.0, rect.max.0, dims.0
            ),
            SeamError::LandsOutside { cell, dims } => write!(
                f,
                "the seam would lead to voxel {:?}, outside the destination room ({:?} voxels)",
                cell.at.0, dims.0
            ),
            SeamError::FacesDoNotMeet {
                leave,
                turned,
                needed,
            } => write!(
                f,
                "leaving by {leave:?}, this turn has you travelling {turned:?}, but to arrive at the far side you must travel {needed:?}. \
                 Pick a turn that takes {leave:?} to {needed:?}"
            ),
            SeamError::ShapesDiffer { turned, far } => write!(
                f,
                "the near rectangle, turned, is {:?} voxels and the far one is {:?}. They must be the same shape",
                turned.0, far.0
            ),
            SeamError::Overlap {
                cell,
                face,
                existing,
            } => write!(
                f,
                "face {face:?} of voxel {:?} already belongs to seam {}. A face can lead to one place",
                cell.at.0, existing.0
            ),
            SeamError::MirrorNotFlat { rect } => write!(
                f,
                "a mirror facing {:?} must be one voxel thick along that axis; this rectangle is {:?} voxels",
                rect.face,
                rect.size().0
            ),
            SeamError::UpAlongTheWay => {
                write!(
                    f,
                    "\"up\" cannot be the direction you walk through the opening; give a face on another axis"
                )
            }
        }
    }
}

impl std::error::Error for SeamError {}

/// A grid of voxels with its own coordinates. Material 0 is empty.
#[derive(Clone, Debug)]
pub struct Room {
    name: String,
    dims: IVec3,
    material: Vec<u8>,
    linked: Vec<u8>,
}

impl Room {
    /// The name it was added with.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Size in voxels along x, y and z.
    pub fn dims(&self) -> IVec3 {
        self.dims
    }

    /// True if `c` is inside the room.
    pub fn contains(&self, c: IVec3) -> bool {
        (0..3).all(|i| c[i] >= 0 && c[i] < self.dims[i])
    }

    pub(crate) fn index(&self, c: IVec3) -> usize {
        (c[0] + self.dims[0] * (c[1] + self.dims[1] * c[2])) as usize
    }

    /// The material at `c`. Outside the room is empty.
    pub fn material(&self, c: IVec3) -> u8 {
        if self.contains(c) {
            self.material[self.index(c)]
        } else {
            0
        }
    }

    /// True if there is something at `c`.
    pub fn is_solid(&self, c: IVec3) -> bool {
        self.material(c) != 0
    }

    /// Put a material at `c`. Outside the room does nothing.
    pub fn set(&mut self, c: IVec3, material: u8) {
        if self.contains(c) {
            let i = self.index(c);
            self.material[i] = material;
        }
    }

    /// Fill a box (corners inclusive, in either order), clipped to the room.
    pub fn fill(&mut self, a: IVec3, b: IVec3, material: u8) {
        let lo = a.min(b).max(IVec3::ZERO);
        let hi = a.max(b).min(self.dims - ivec3(1, 1, 1));
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let i = self.index(ivec3(x, y, z));
                    self.material[i] = material;
                }
            }
        }
    }

    pub(crate) fn material_at(&self, index: usize) -> u8 {
        self.material[index]
    }

    pub(crate) fn linked_at(&self, index: usize) -> u8 {
        self.linked[index]
    }
}

fn cells_of(rect: FaceRect) -> impl Iterator<Item = IVec3> {
    (rect.min[2]..=rect.max[2]).flat_map(move |z| {
        (rect.min[1]..=rect.max[1])
            .flat_map(move |y| (rect.min[0]..=rect.max[0]).map(move |x| ivec3(x, y, z)))
    })
}

/// Rooms and the seams between them.
#[derive(Clone, Debug, Default)]
pub struct World {
    rooms: Vec<Room>,
    seams: Vec<Seam>,
    by_room: Vec<Vec<SeamId>>,
}

impl World {
    /// An empty world.
    pub fn new() -> World {
        World::default()
    }

    /// Add an empty room of the given size.
    ///
    /// # Panics
    /// If any dimension is less than one.
    pub fn add_room(&mut self, name: &str, dims: IVec3) -> RoomId {
        assert!(
            (0..3).all(|i| dims[i] >= 1),
            "a room needs at least one voxel along each axis"
        );
        let n = (dims[0] as usize) * (dims[1] as usize) * (dims[2] as usize);
        self.rooms.push(Room {
            name: name.to_owned(),
            dims,
            material: vec![0; n],
            linked: vec![0; n],
        });
        self.by_room.push(Vec::new());
        RoomId(self.rooms.len() as u32 - 1)
    }

    /// A room.
    pub fn room(&self, id: RoomId) -> &Room {
        &self.rooms[id.0 as usize]
    }

    /// A room, to build in.
    pub fn room_mut(&mut self, id: RoomId) -> &mut Room {
        &mut self.rooms[id.0 as usize]
    }

    /// Every room, in the order added.
    pub fn rooms(&self) -> &[Room] {
        &self.rooms
    }

    /// A seam.
    pub fn seam(&self, id: SeamId) -> &Seam {
        &self.seams[id.0 as usize]
    }

    /// Every seam, in the order made.
    pub fn seams(&self) -> &[Seam] {
        &self.seams
    }

    /// The seam on this face of this voxel, if there is one.
    pub fn seam_at(&self, cell: Cell, face: Face) -> Option<SeamId> {
        let room = self.room(cell.room);
        if !room.contains(cell.at) || room.linked[room.index(cell.at)] & face.bit() == 0 {
            return None;
        }
        self.find_seam(cell, face)
    }

    pub(crate) fn find_seam(&self, cell: Cell, face: Face) -> Option<SeamId> {
        self.by_room[cell.room.0 as usize]
            .iter()
            .copied()
            .find(|id| {
                let s = &self.seams[id.0 as usize];
                s.from.face == face && s.from.contains(cell.at)
            })
    }

    /// Make a one-way seam: leaving any voxel of `from` through `from.face`
    /// puts you in room `to`, at the place `map` gives for the voxel you were
    /// stepping into.
    ///
    /// Most worlds want [`glue`](World::glue), which makes both directions
    /// and works the map out.
    pub fn link(
        &mut self,
        from: FaceRect,
        to: RoomId,
        map: LatticeMap,
        kind: SeamKind,
    ) -> Result<SeamId, SeamError> {
        self.check(from, to, map)?;
        Ok(self.commit(from, to, map, kind))
    }

    /// Everything that can be wrong with a seam, checked without changing
    /// anything.
    fn check(&self, from: FaceRect, to: RoomId, map: LatticeMap) -> Result<(), SeamError> {
        if (0..3).any(|i| from.min[i] > from.max[i]) {
            return Err(SeamError::EmptyRect { rect: from });
        }
        let room = self.room(from.room);
        if !room.contains(from.min) || !room.contains(from.max) {
            return Err(SeamError::OutsideRoom {
                rect: from,
                dims: room.dims,
            });
        }
        // A box maps to a box, so checking two opposite corners checks them all.
        let n = from.face.normal();
        for corner in [from.min, from.max] {
            let lands = map.cell(corner + n);
            if !self.room(to).contains(lands) {
                return Err(SeamError::LandsOutside {
                    cell: Cell {
                        room: to,
                        at: lands,
                    },
                    dims: self.room(to).dims,
                });
            }
        }
        for at in cells_of(from) {
            if room.linked[room.index(at)] & from.face.bit() != 0 {
                let cell = Cell {
                    room: from.room,
                    at,
                };
                let existing = self
                    .find_seam(cell, from.face)
                    .expect("a linked face has a seam");
                return Err(SeamError::Overlap {
                    cell,
                    face: from.face,
                    existing,
                });
            }
        }
        Ok(())
    }

    fn commit(&mut self, from: FaceRect, to: RoomId, map: LatticeMap, kind: SeamKind) -> SeamId {
        let id = SeamId(self.seams.len() as u32);
        self.seams.push(Seam {
            from,
            to,
            map,
            kind,
        });
        self.by_room[from.room.0 as usize].push(id);
        let room = self.room_mut(from.room);
        for at in cells_of(from) {
            let i = room.index(at);
            room.linked[i] |= from.face.bit();
        }
        id
    }

    /// Make `a` and `b` the two sides of one opening.
    ///
    /// `a` is the voxels you leave by and `b` the voxels you arrive in; each
    /// rectangle's face points out through the opening. `turn` says how
    /// directions in `a`'s room read in `b`'s room. It must send `a.face` to
    /// the opposite of `b.face` (walking out of one side is walking into the
    /// other) and must send the shape of `a` to the shape of `b`.
    ///
    /// Returns the seam from `a` and the seam back from `b`. Their maps are
    /// inverses: one change of coordinates, read both ways.
    pub fn glue(
        &mut self,
        a: FaceRect,
        b: FaceRect,
        turn: Orientation,
    ) -> Result<[SeamId; 2], SeamError> {
        let turned = turn.face(a.face);
        if turned != b.face.opposite() {
            return Err(SeamError::FacesDoNotMeet {
                leave: a.face,
                turned,
                needed: b.face.opposite(),
            });
        }
        if (0..3).any(|i| a.min[i] > a.max[i]) {
            return Err(SeamError::EmptyRect { rect: a });
        }
        if (0..3).any(|i| b.min[i] > b.max[i]) {
            return Err(SeamError::EmptyRect { rect: b });
        }
        let turned_size = turn.apply(a.size()).abs();
        if turned_size != b.size() {
            return Err(SeamError::ShapesDiffer {
                turned: turned_size,
                far: b.size(),
            });
        }
        // Slide the turned box of `a` onto the box of `b`: that fixes where
        // each voxel of `a` would be if it were in `b`'s room. The voxel you
        // step into from `a` is one further out, and must land in `b`.
        let low = turn.apply(a.min).min(turn.apply(a.max));
        let offset = b.min - low - turn.apply(a.face.normal());
        let map = LatticeMap { rot: turn, offset };
        // Both directions are checked before either is made, so a glue that
        // fails leaves the world as it was.
        self.check(a, b.room, map)?;
        self.check(b, a.room, map.inverse())?;
        Ok([
            self.commit(a, b.room, map, SeamKind::Open),
            self.commit(b, a.room, map.inverse(), SeamKind::Open),
        ])
    }

    /// [`glue`](World::glue), with the turn worked out from which way is up
    /// on each side: walking out of `a` you walk into `b`, and `up_a`
    /// becomes `up_b`.
    pub fn glue_up(
        &mut self,
        a: FaceRect,
        b: FaceRect,
        up_a: Face,
        up_b: Face,
    ) -> Result<[SeamId; 2], SeamError> {
        let turn = Orientation::from_pairs(a.face, b.face.opposite(), up_a, up_b)
            .ok_or(SeamError::UpAlongTheWay)?;
        self.glue(a, b, turn)
    }

    /// Make a room endless along one axis: its far face opens onto its near
    /// face.
    pub fn wrap(&mut self, room: RoomId, axis: usize) -> Result<[SeamId; 2], SeamError> {
        let d = self.room(room).dims;
        let top = d - ivec3(1, 1, 1);
        let mut far_min = IVec3::ZERO;
        far_min[axis] = top[axis];
        let mut near_max = top;
        near_max[axis] = 0;
        self.glue(
            FaceRect {
                room,
                min: far_min,
                max: top,
                face: Face::new(axis, true),
            },
            FaceRect {
                room,
                min: IVec3::ZERO,
                max: near_max,
                face: Face::new(axis, false),
            },
            Orientation::IDENTITY,
        )
    }

    /// Turn a rectangle of faces into a mirror: each leads back to its own
    /// voxel, reflected.
    pub fn mirror(&mut self, rect: FaceRect) -> Result<SeamId, SeamError> {
        // The voxel beyond the glass must map to the voxel in front of it.
        let axis = rect.face.axis();
        let mut offset = IVec3::ZERO;
        offset[axis] = 2 * rect.min[axis] + rect.face.sign();
        if rect.min[axis] != rect.max[axis] {
            return Err(SeamError::MirrorNotFlat { rect });
        }
        self.link(
            rect,
            rect.room,
            LatticeMap {
                rot: Orientation::mirror(axis),
                offset,
            },
            SeamKind::SightOnly,
        )
    }

    /// The voxel through `face` of `cell`, and the seam crossed to reach it,
    /// if any. `None` when that is outside the room and nothing leads on,
    /// and when `cell` is itself outside its room.
    pub fn neighbour(&self, cell: Cell, face: Face) -> Option<(Cell, Option<SeamId>)> {
        let room = self.room(cell.room);
        if !room.contains(cell.at) {
            return None;
        }
        let beyond = cell.at + face.normal();
        if room.linked[room.index(cell.at)] & face.bit() != 0 {
            if let Some(id) = self.find_seam(cell, face) {
                let s = &self.seams[id.0 as usize];
                return Some((
                    Cell {
                        room: s.to,
                        at: s.map.cell(beyond),
                    },
                    Some(id),
                ));
            }
        }
        room.contains(beyond).then_some((
            Cell {
                room: cell.room,
                at: beyond,
            },
            None,
        ))
    }

    /// Follow the lattice `delta` voxels from `start`: along x, then y, then
    /// z, where `delta` is in the frame `start` is in. Returns where that
    /// leads and the turn picked up on the way.
    ///
    /// Around the rim of a seam space is not flat, and a different order of
    /// axes can reach a different voxel. For short hops through the middle
    /// of an opening every order agrees.
    pub fn walk(&self, start: Cell, delta: IVec3) -> Option<(Cell, Orientation)> {
        self.walk_through(start, delta, |_| true)
    }

    /// [`walk`](World::walk) for solid things: a seam that only sight
    /// crosses (a mirror) is a wall, and a walk that would go through one
    /// leads nowhere.
    ///
    /// Use this to find where a body's box or a figure's voxels are. With
    /// `walk`, the voxel "beyond" a mirror is the reflected room, which is
    /// right for what a ray sees there and wrong for what stands there.
    pub fn walk_open(&self, start: Cell, delta: IVec3) -> Option<(Cell, Orientation)> {
        self.walk_through(start, delta, |seam| seam.kind == SeamKind::Open)
    }

    fn walk_through(
        &self,
        start: Cell,
        delta: IVec3,
        passes: impl Fn(&Seam) -> bool,
    ) -> Option<(Cell, Orientation)> {
        let mut cell = start;
        let mut turn = Orientation::IDENTITY;
        for axis in 0..3 {
            let dir = Face::new(axis, delta[axis] > 0);
            for _ in 0..delta[axis].abs() {
                let (next, seam) = self.neighbour(cell, turn.face(dir))?;
                if let Some(id) = seam {
                    let seam = &self.seams[id.0 as usize];
                    if !passes(seam) {
                        return None;
                    }
                    turn = turn.then(seam.map.rot);
                }
                cell = next;
            }
        }
        Some((cell, turn))
    }
}
