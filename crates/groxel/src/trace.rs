//! Moving in a straight line through the world, one face at a time.
//!
//! [`Cursor`] is the only traversal loop in the crate. Rays, poses and custom
//! queries all advance one, so all of them cross seams the same way. When a
//! cursor crosses a seam its per-axis stepping state is shuffled by the
//! seam's turn and nothing is recomputed, which is why a line through any
//! number of seams visits the voxels, at the distances, that the same line
//! visits in the world with the seams unrolled.
//!
//! The distance to the next face on an axis is worked out as
//! `first + count * delta`. The usual form, a running total that has `delta`
//! added at each step, rounds at each addition, and over a ray thousands of
//! voxels long the roundings add up to a tenth of a voxel.

use crate::orient::{Face, Orientation};
use crate::overlay::{EntityId, Overlay, Skip, Space, Stamp};
use crate::vec::{IVec3, Vec3};
use crate::world::{Cell, RoomId, SeamId, World};
use std::ops::ControlFlow;

/// A half-line: where it starts and which way it goes.
#[derive(Copy, Clone, Debug)]
pub struct Ray {
    /// The voxel the origin is in. Stated, not derived, because a point on a
    /// boundary belongs to whichever voxel you say.
    pub start: Cell,
    /// The starting point, in the room's frame.
    pub origin: Vec3,
    /// The direction. Distances are in units of its length, so use a unit
    /// vector for distances in voxels.
    pub dir: Vec3,
}

impl Ray {
    /// A ray from a point, in the voxel that point is in.
    pub fn new(room: RoomId, origin: Vec3, dir: Vec3) -> Ray {
        Ray {
            start: Cell {
                room,
                at: origin.floor(),
            },
            origin,
            dir,
        }
    }
}

/// What a cursor is about to do: the next face it will cross.
#[derive(Copy, Clone, Debug)]
pub struct Ahead {
    /// Distance along the ray at which the face is reached.
    pub t: f32,
    /// The face of the current voxel.
    pub face: Face,
    /// The seam on that face, if any.
    pub seam: Option<SeamId>,
    /// True if crossing would leave the room with nothing leading on.
    pub leaves: bool,
}

/// What a cursor just did.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Moved {
    /// It is in a new voxel.
    Entered {
        /// The face of the new voxel it came in through.
        through: Face,
        /// The seam it crossed, if any.
        via: Option<SeamId>,
    },
    /// The next voxel would be outside the room, or the next face is
    /// infinitely far (a ray with no direction never reaches one). The
    /// cursor has not moved.
    Left,
}

/// A point travelling along a straight line, voxel by voxel, through seams.
#[derive(Clone, Debug)]
pub struct Cursor<'w> {
    world: &'w World,
    overlay: Option<&'w Overlay>,
    room: RoomId,
    cell: IVec3,
    index: usize,
    origin: Vec3,
    dir: Vec3,
    step: [i32; 3],
    t_max: [f32; 3],
    t_delta: [f32; 3],
    /// Distance to the first face on each axis, and faces crossed since.
    t_first: [f32; 3],
    count: [u32; 3],
    /// Which of the starting frame's axes each current axis was.
    rank: [u8; 3],
    t: f32,
    frame: Orientation,
    crossings: u32,
}

impl<'w> Cursor<'w> {
    /// Start at the ray's origin.
    ///
    /// # Panics
    /// If the ray's starting voxel is outside its room.
    pub fn new(space: &'w (impl Space + ?Sized), ray: Ray) -> Cursor<'w> {
        let world = space.world();
        let room = world.room(ray.start.room);
        assert!(
            room.contains(ray.start.at),
            "a ray must start inside its room"
        );
        let cell = ray.start.at;
        let mut step = [0; 3];
        let mut t_max = [f32::INFINITY; 3];
        let mut t_delta = [f32::INFINITY; 3];
        for i in 0..3 {
            let lo = cell[i] as f32;
            let d = ray.dir[i];
            if d > 0.0 {
                step[i] = 1;
                t_delta[i] = 1.0 / d;
                t_max[i] = ((lo + 1.0 - ray.origin[i]) * t_delta[i]).max(0.0);
            } else if d < 0.0 {
                step[i] = -1;
                t_delta[i] = -1.0 / d;
                t_max[i] = ((ray.origin[i] - lo) * t_delta[i]).max(0.0);
            }
        }
        Cursor {
            world,
            overlay: space.overlay(),
            room: ray.start.room,
            cell,
            index: room.index(cell),
            origin: ray.origin,
            dir: ray.dir,
            step,
            t_max,
            t_delta,
            t_first: t_max,
            count: [0; 3],
            rank: [0, 1, 2],
            t: 0.0,
            frame: Orientation::IDENTITY,
            crossings: 0,
        }
    }

    /// The voxel the cursor is in.
    pub fn cell(&self) -> Cell {
        Cell {
            room: self.room,
            at: self.cell,
        }
    }

    /// Distance travelled to the last face crossed.
    pub fn t(&self) -> f32 {
        self.t
    }

    /// The ray's origin as it reads in the current room's frame. The point
    /// at distance `t` is `origin() + dir() * t`, in that frame.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// The ray's direction in the current room's frame.
    pub fn dir(&self) -> Vec3 {
        self.dir
    }

    /// The turn from the frame the ray started in to the current frame.
    pub fn frame(&self) -> Orientation {
        self.frame
    }

    /// How many seams have been crossed.
    pub fn crossings(&self) -> u32 {
        self.crossings
    }

    /// The material of the voxel the cursor is in: the world's, or failing
    /// that the overlay's.
    #[inline]
    pub fn material(&self) -> u8 {
        let material = self.world.room(self.room).material_at(self.index);
        if material != 0 || self.overlay.is_none() {
            return material;
        }
        self.stamped().map_or(0, |stamp| stamp.material)
    }

    /// The overlay voxel the cursor is in, if the world is empty there and
    /// something is stamped.
    pub fn stamp(&self) -> Option<Stamp> {
        if self.world.room(self.room).material_at(self.index) != 0 {
            return None;
        }
        self.stamped()
    }

    #[cold]
    fn stamped(&self) -> Option<Stamp> {
        self.overlay?.at(self.room, self.index)
    }

    /// The axis whose face comes next. When two faces are equally near (the
    /// line passes through an edge or a corner) the axis that was later in
    /// the starting frame goes first. Deciding by the starting frame, not the
    /// current one, is what makes a line visit the same voxels whether or
    /// not there are seams on its way.
    fn next_axis(&self) -> usize {
        let (m, r) = (&self.t_max, &self.rank);
        // Ties are rare, so the rank is read only when one happens.
        let nearer = |a: usize, b: usize| {
            if m[a] < m[b] || (m[a] == m[b] && r[a] > r[b]) {
                a
            } else {
                b
            }
        };
        nearer(nearer(0, 1), 2)
    }

    /// Distance at which the next face is crossed.
    pub fn next_t(&self) -> f32 {
        self.t_max[self.next_axis()]
    }

    /// The next face, without crossing it.
    pub fn ahead(&self) -> Ahead {
        let axis = self.next_axis();
        let face = Face::new(axis, self.step[axis] > 0);
        let room = self.world.room(self.room);
        let seam = if room.linked_at(self.index) & face.bit() != 0 {
            self.world.find_seam(self.cell(), face)
        } else {
            None
        };
        let leaves = seam.is_none() && !room.contains(self.cell + face.normal());
        Ahead {
            t: self.t_max[axis],
            face,
            seam,
            leaves,
        }
    }

    /// Record the crossing of the next face on `axis` and work out the
    /// distance to the one after it. Counting faces and multiplying keeps
    /// the error of that distance at one rounding however long the ray is.
    #[inline]
    fn step_along(&mut self, axis: usize) {
        self.t = self.t_max[axis];
        self.count[axis] += 1;
        self.t_max[axis] = self.t_first[axis] + self.count[axis] as f32 * self.t_delta[axis];
    }

    /// Cross the next face.
    pub fn advance(&mut self) -> Moved {
        let axis = self.next_axis();
        // A face at infinity is not crossed. Without this a ray with no
        // direction would stay put in a plain room and go through a seam
        // beside it, and the two would no longer be the same ray.
        if self.t_max[axis] == f32::INFINITY {
            return Moved::Left;
        }
        let face = Face::new(axis, self.step[axis] > 0);
        let room = self.world.room(self.room);

        if room.linked_at(self.index) & face.bit() != 0 {
            if let Some(id) = self.world.find_seam(self.cell(), face) {
                let seam = self.world.seam(id);
                let map = seam.map;
                self.step_along(axis);

                // Everything about the ray is re-read in the new frame.
                self.cell = map.cell(self.cell + face.normal());
                self.origin = map.point(self.origin);
                self.dir = map.dir(self.dir);
                let (step, t_max, t_delta, rank) = (self.step, self.t_max, self.t_delta, self.rank);
                let (t_first, count) = (self.t_first, self.count);
                for j in 0..3 {
                    let (src, sign) = map.rot.source(j);
                    self.step[j] = step[src] * sign;
                    self.t_max[j] = t_max[src];
                    self.t_delta[j] = t_delta[src];
                    self.t_first[j] = t_first[src];
                    self.count[j] = count[src];
                    self.rank[j] = rank[src];
                }
                self.room = seam.to;
                self.index = self.world.room(self.room).index(self.cell);
                self.frame = self.frame.then(map.rot);
                self.crossings += 1;
                return Moved::Entered {
                    through: map.rot.face(face).opposite(),
                    via: Some(id),
                };
            }
        }

        let next = self.cell[axis] + self.step[axis];
        if next < 0 || next >= room.dims()[axis] {
            return Moved::Left;
        }
        self.step_along(axis);
        self.cell[axis] = next;
        self.index = room.index(self.cell);
        Moved::Entered {
            through: face.opposite(),
            via: None,
        }
    }
}

/// How a trace ended.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HitKind {
    /// It reached a solid voxel.
    Solid,
    /// It left a room through a face that leads nowhere.
    Sky,
    /// It ran out of distance or steps.
    Far,
}

/// Limits on a trace.
#[derive(Copy, Clone, Debug)]
pub struct TraceOpts {
    /// Give up beyond this distance.
    pub max_t: f32,
    /// Give up after this many voxels.
    pub max_steps: u32,
    /// Look past one entity's overlay voxels near the start of the ray.
    pub skip: Option<Skip>,
}

impl Default for TraceOpts {
    fn default() -> Self {
        TraceOpts {
            max_t: 400.0,
            max_steps: 4000,
            skip: None,
        }
    }
}

/// The result of a trace.
#[derive(Copy, Clone, Debug)]
pub struct Hit {
    /// How it ended.
    pub kind: HitKind,
    /// The solid voxel, or the last voxel the ray was in.
    pub cell: Cell,
    /// For a solid hit: the face of that voxel the ray came in through.
    pub face: Face,
    /// For a solid hit: its material.
    pub material: u8,
    /// For a solid hit on an overlay voxel: whose it is.
    pub entity: Option<EntityId>,
    /// Distance travelled.
    pub t: f32,
    /// The ray's origin as it reads in `cell`'s room.
    pub origin: Vec3,
    /// The ray's direction as it reads in `cell`'s room.
    pub dir: Vec3,
    /// The turn from the ray's starting frame to `cell`'s room.
    pub frame: Orientation,
    /// The empty voxel the ray was in just before a solid hit.
    pub from: Cell,
    /// The seam crossed on the last step, if that step crossed one.
    pub via: Option<SeamId>,
    /// Seams crossed in all.
    pub crossings: u32,
    /// Voxels visited.
    pub steps: u32,
}

impl Hit {
    /// Where the ray ended, in the frame of `cell`'s room.
    pub fn point(&self) -> Vec3 {
        self.origin + self.dir * self.t
    }
}

/// Follow a ray to the first solid voxel.
pub fn trace(space: &(impl Space + ?Sized), ray: Ray, opts: &TraceOpts) -> Hit {
    let mut cur = Cursor::new(space, ray);
    let mut steps = 0;
    let done = |cur: &Cursor, kind, face, from, via, t, steps| Hit {
        kind,
        cell: cur.cell(),
        face,
        material: if kind == HitKind::Solid {
            cur.material()
        } else {
            0
        },
        entity: if kind == HitKind::Solid {
            cur.stamp().map(|stamp| stamp.entity)
        } else {
            None
        },
        t,
        origin: cur.origin,
        dir: cur.dir,
        frame: cur.frame,
        from,
        via,
        crossings: cur.crossings,
        steps,
    };
    loop {
        let next_t = cur.next_t();
        if next_t > opts.max_t || next_t == f32::INFINITY || steps >= opts.max_steps {
            let here = cur.cell();
            return done(
                &cur,
                HitKind::Far,
                Face::PosX,
                here,
                None,
                next_t.min(opts.max_t),
                steps,
            );
        }
        let from = cur.cell();
        match cur.advance() {
            Moved::Left => return done(&cur, HitKind::Sky, Face::PosX, from, None, next_t, steps),
            Moved::Entered { through, via } => {
                steps += 1;
                if cur.material() != 0 {
                    // The viewer's own voxels do not count until the ray
                    // has gone some way. The rule reads the distance and
                    // nothing else, so that it cannot tell a world from the
                    // same world cut and glued.
                    let passed = match (opts.skip, cur.stamp()) {
                        (Some(skip), Some(stamp)) => {
                            stamp.entity == skip.entity && cur.t < skip.near
                        }
                        _ => false,
                    };
                    if !passed {
                        return done(&cur, HitKind::Solid, through, from, via, cur.t, steps);
                    }
                }
            }
        }
    }
}

/// Walk a ray through the world and call `f` for each voxel it enters, until
/// `f` breaks, the ray leaves, or `max_t` is passed. Solid voxels do not stop
/// it: that is `f`'s decision.
///
/// This is the tool for line of sight, sound, picking and anything else that
/// is not "first solid voxel", so that no traversal loop needs writing
/// outside this crate.
pub fn visit(
    space: &(impl Space + ?Sized),
    ray: Ray,
    max_t: f32,
    mut f: impl FnMut(&Cursor, Moved) -> ControlFlow<()>,
) {
    let mut cur = Cursor::new(space, ray);
    while cur.next_t() <= max_t && cur.next_t() < f32::INFINITY {
        let moved = cur.advance();
        if moved == Moved::Left || f(&cur, moved).is_break() {
            return;
        }
    }
}
