//! Shared by the measurement programs in `src/bin`.
//!
//! The test world is a cube of `n` voxels a side with every face glued to the
//! opposite one: x and y by plain wraps, and z with a quarter turn. That is
//! the quarter-turn space of Conway and Rossetti with a voxel lattice in it.
//! A ray in it never leaves, so it can cross as many seams as a measurement
//! wants.
//!
//! Ways of carrying a ray across a seam are compared with two references:
//!
//! - [`unrolled`] traces the same ray, in single precision, through the
//!   unrolled world: all of space tiled with turned copies of the cube. It
//!   never crosses anything. It looks a voxel up by folding its integer
//!   coordinates back into the cube, which is exact. This is what an engine
//!   with no seams would compute.
//! - [`truth`] does the same in double precision, and stands for where the
//!   ray really goes.
//!
//! The ways of crossing:
//!
//! - [`engine`] is the engine's own cursor, which permutes its stepping state
//!   and counts its steps.
//! - [`permuting`] is the same rule written here against the public API, so
//!   that it can be run with either form of stepping state ([`Running`] or
//!   [`Counted`]) and timed beside the baselines on equal terms.
//! - [`recompute`] is how earlier voxel tracers cross a portal: transform the
//!   ray, then work the stepping state out again from the new position. Its
//!   options select the variants found in those tracers.
//! - [`cellwise`] is HyperRogue's rule: no stepping state at all. The ray is
//!   a position and a tangent in the coordinates of the cell it is in, and a
//!   matrix carries both through every wall.

use groxel::{
    Cell, Cursor, Face, FaceRect, IVec3, LatticeMap, Moved, Orientation, Ray, RoomId, Vec3, World,
    ivec3, vec3,
};

/// A small deterministic generator, so the measurements need no dependencies
/// and repeat exactly from their seeds.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform in 0..1.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in lo..hi.
    pub fn below(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next_u64() % (hi - lo) as u64) as i32
    }

    /// A unit vector, uniform over the sphere.
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

/// The quarter-turn space as a groxel world.
pub struct ScrewWorld {
    pub world: World,
    pub room: RoomId,
    pub n: i32,
    /// The map of the seam on the +z face.
    pub screw: LatticeMap,
}

impl ScrewWorld {
    /// An empty cube of side `n`, glued up.
    pub fn new(n: i32) -> ScrewWorld {
        let mut world = World::new();
        let room = world.add_room("quarter-turn space", ivec3(n, n, n));
        world.wrap(room, 0).expect("x wraps");
        world.wrap(room, 1).expect("y wraps");
        let top = n - 1;
        let [up, _down] = world
            .glue(
                FaceRect {
                    room,
                    min: ivec3(0, 0, top),
                    max: ivec3(top, top, top),
                    face: Face::PosZ,
                },
                FaceRect {
                    room,
                    min: ivec3(0, 0, 0),
                    max: ivec3(top, top, 0),
                    face: Face::NegZ,
                },
                Orientation::quarter_turn(2, 1),
            )
            .expect("z glues with a quarter turn");
        let screw = world.seam(up).map;
        // Four turns of the screw are a plain shift of four cubes. `fold`
        // relies on it.
        let four = screw.then(screw).then(screw).then(screw);
        assert_eq!(four, LatticeMap::shift(ivec3(0, 0, -4 * n)));
        ScrewWorld {
            world,
            room,
            n,
            screw,
        }
    }

    /// Scatter solid voxels with the given density.
    pub fn scatter(&mut self, rng: &mut Rng, density: f32) {
        let n = self.n;
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    if rng.unit() < density {
                        let m = rng.below(1, 255) as u8;
                        self.world.room_mut(self.room).set(ivec3(x, y, z), m);
                    }
                }
            }
        }
    }

    /// Fold a voxel of the unrolled world back into the cube: the voxel of
    /// the room it is a copy of, and the turn from the unrolled frame to the
    /// room's frame there. Integer arithmetic only.
    pub fn fold(&self, c: IVec3) -> (IVec3, Orientation) {
        let n = self.n;
        let k = c[2].div_euclid(n);
        let mut at = c - ivec3(0, 0, 4 * n * k.div_euclid(4));
        let mut turn = Orientation::IDENTITY;
        for _ in 0..k.rem_euclid(4) {
            at = self.screw.cell(at);
            turn = turn.then(self.screw.rot);
        }
        at[0] = at[0].rem_euclid(n);
        at[1] = at[1].rem_euclid(n);
        debug_assert!((0..n).contains(&at[2]));
        (at, turn)
    }

    pub fn material(&self, at: IVec3) -> u8 {
        self.world.room(self.room).material(at)
    }
}

/// Where a ray ended.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Outcome {
    /// False if the step limit ran out first.
    pub hit: bool,
    /// The solid voxel, in the room.
    pub cell: IVec3,
    /// The face of it the ray came in through, in the room's frame.
    pub face: Face,
    pub material: u8,
    /// Distance along the ray.
    pub t: f32,
    /// Voxels entered.
    pub steps: u32,
    /// Seams crossed.
    pub crossings: u32,
}

impl Outcome {
    /// The same in every field, with the distance compared bit for bit.
    pub fn identical(&self, o: &Outcome) -> bool {
        self.hit == o.hit
            && self.cell == o.cell
            && self.face == o.face
            && self.material == o.material
            && self.t.to_bits() == o.t.to_bits()
            && self.steps == o.steps
            && self.crossings == o.crossings
    }
}

const MISS: Outcome = Outcome {
    hit: false,
    cell: IVec3::ZERO,
    face: Face::PosX,
    material: 0,
    t: f32::INFINITY,
    steps: 0,
    crossings: 0,
};

/// The Amanatides-Woo stepping state, in one of two forms.
pub trait Stepper: Copy {
    /// The state for a ray at `origin` in voxel `cell`.
    fn new(origin: Vec3, dir: Vec3, cell: IVec3) -> Self;
    /// The axis whose face is nearest. A tie goes to the axis that was later
    /// in the frame the ray started in, which is the engine's rule.
    fn next_axis(&self) -> usize;
    /// Which way the ray steps along `axis`: -1, 0 or 1.
    fn step(&self, axis: usize) -> i32;
    /// Cross the face on `axis`; returns the distance at which it was.
    fn advance(&mut self, axis: usize) -> f32;
    /// The same state read in the frame a seam leads to. Each axis's values
    /// move to the axis the turn sends it to, and nothing is recalculated.
    fn turned(&self, rot: Orientation) -> Self;
}

fn nearest(t_max: &[f32; 3], rank: &[u8; 3]) -> usize {
    let nearer = |a: usize, b: usize| {
        if t_max[a] < t_max[b] || (t_max[a] == t_max[b] && rank[a] > rank[b]) {
            a
        } else {
            b
        }
    };
    nearer(nearer(0, 1), 2)
}

/// The usual form: the distance to the next face on an axis is a running
/// total, `t_max += t_delta`. The engine's cursor does not use this form,
/// because these measurements show that it drifts.
#[derive(Copy, Clone)]
pub struct Running {
    step: [i32; 3],
    t_max: [f32; 3],
    t_delta: [f32; 3],
    rank: [u8; 3],
}

impl Stepper for Running {
    fn new(origin: Vec3, dir: Vec3, cell: IVec3) -> Running {
        let mut s = Running {
            step: [0; 3],
            t_max: [f32::INFINITY; 3],
            t_delta: [f32::INFINITY; 3],
            rank: [0, 1, 2],
        };
        // Written as the engine's cursor writes it.
        for i in 0..3 {
            let lo = cell[i] as f32;
            let d = dir[i];
            if d > 0.0 {
                s.step[i] = 1;
                s.t_delta[i] = 1.0 / d;
                s.t_max[i] = ((lo + 1.0 - origin[i]) * s.t_delta[i]).max(0.0);
            } else if d < 0.0 {
                s.step[i] = -1;
                s.t_delta[i] = -1.0 / d;
                s.t_max[i] = ((origin[i] - lo) * s.t_delta[i]).max(0.0);
            }
        }
        s
    }

    fn next_axis(&self) -> usize {
        nearest(&self.t_max, &self.rank)
    }

    fn step(&self, axis: usize) -> i32 {
        self.step[axis]
    }

    fn advance(&mut self, axis: usize) -> f32 {
        let t = self.t_max[axis];
        self.t_max[axis] += self.t_delta[axis];
        t
    }

    fn turned(&self, rot: Orientation) -> Running {
        let mut out = *self;
        for j in 0..3 {
            let (src, sign) = rot.source(j);
            out.step[j] = self.step[src] * sign;
            out.t_max[j] = self.t_max[src];
            out.t_delta[j] = self.t_delta[src];
            out.rank[j] = self.rank[src];
        }
        out
    }
}

/// Counted steps: the distance to the next face on an axis is
/// `first + count * delta`, worked out afresh each time. A running total's
/// rounding errors add up along a ray. This form's do not. This is what the
/// engine's cursor holds.
#[derive(Copy, Clone)]
pub struct Counted {
    step: [i32; 3],
    first: [f32; 3],
    delta: [f32; 3],
    count: [u32; 3],
    t_max: [f32; 3],
    rank: [u8; 3],
}

impl Stepper for Counted {
    fn new(origin: Vec3, dir: Vec3, cell: IVec3) -> Counted {
        let s = Running::new(origin, dir, cell);
        Counted {
            step: s.step,
            first: s.t_max,
            delta: s.t_delta,
            count: [0; 3],
            t_max: s.t_max,
            rank: s.rank,
        }
    }

    fn next_axis(&self) -> usize {
        nearest(&self.t_max, &self.rank)
    }

    fn step(&self, axis: usize) -> i32 {
        self.step[axis]
    }

    fn advance(&mut self, axis: usize) -> f32 {
        let t = self.t_max[axis];
        self.count[axis] += 1;
        self.t_max[axis] = self.first[axis] + self.count[axis] as f32 * self.delta[axis];
        t
    }

    fn turned(&self, rot: Orientation) -> Counted {
        let mut out = *self;
        for j in 0..3 {
            let (src, sign) = rot.source(j);
            out.step[j] = self.step[src] * sign;
            out.first[j] = self.first[src];
            out.delta[j] = self.delta[src];
            out.count[j] = self.count[src];
            out.t_max[j] = self.t_max[src];
            out.rank[j] = self.rank[src];
        }
        out
    }
}

/// The ray in the unrolled world. No seams: a plain Amanatides-Woo walk over
/// all of space, with each voxel looked up by folding its coordinates.
///
/// Solid voxels are ignored until the ray has crossed `gate` cube boundaries,
/// which lets a measurement choose how many crossings come before the hit.
pub fn unrolled<S: Stepper>(
    sw: &ScrewWorld,
    origin: Vec3,
    dir: Vec3,
    gate: u32,
    max_steps: u32,
) -> Outcome {
    let n = sw.n;
    let mut cell = origin.floor();
    let mut s = S::new(origin, dir, cell);
    let (mut steps, mut crossings) = (0, 0);
    while steps < max_steps {
        let axis = s.next_axis();
        let t = s.advance(axis);
        let before = cell[axis].div_euclid(n);
        cell[axis] += s.step(axis);
        if cell[axis].div_euclid(n) != before {
            crossings += 1;
        }
        steps += 1;
        if crossings >= gate {
            let (at, turn) = sw.fold(cell);
            let material = sw.material(at);
            if material != 0 {
                let entered = Face::new(axis, s.step(axis) > 0).opposite();
                return Outcome {
                    hit: true,
                    cell: at,
                    face: turn.face(entered),
                    material,
                    t,
                    steps,
                    crossings,
                };
            }
        }
    }
    Outcome {
        steps,
        crossings,
        ..MISS
    }
}

/// Where the ray really goes: the unrolled world again, stepped in double
/// precision from the same single-precision origin and direction. Over the
/// distances measured here its rounding is some nine orders of magnitude
/// below a voxel, so it stands for the exact line. Returns the distance in
/// double precision as well.
pub fn truth(
    sw: &ScrewWorld,
    origin: Vec3,
    dir: Vec3,
    gate: u32,
    max_steps: u32,
) -> (Outcome, f64) {
    let n = sw.n;
    let mut cell = origin.floor();
    let mut step = [0i32; 3];
    let mut t_max = [f64::INFINITY; 3];
    let mut t_delta = [f64::INFINITY; 3];
    for i in 0..3 {
        let (lo, o, d) = (cell[i] as f64, origin[i] as f64, dir[i] as f64);
        if d > 0.0 {
            step[i] = 1;
            t_delta[i] = 1.0 / d;
            t_max[i] = ((lo + 1.0 - o) * t_delta[i]).max(0.0);
        } else if d < 0.0 {
            step[i] = -1;
            t_delta[i] = -1.0 / d;
            t_max[i] = ((o - lo) * t_delta[i]).max(0.0);
        }
    }
    let mut count = [0u32; 3];
    let first = t_max;
    let (mut steps, mut crossings) = (0, 0);
    while steps < max_steps {
        let a = if t_max[0] < t_max[1] { 0 } else { 1 };
        let axis = if t_max[a] < t_max[2] { a } else { 2 };
        let t = t_max[axis];
        count[axis] += 1;
        t_max[axis] = first[axis] + count[axis] as f64 * t_delta[axis];
        let before = cell[axis].div_euclid(n);
        cell[axis] += step[axis];
        if cell[axis].div_euclid(n) != before {
            crossings += 1;
        }
        steps += 1;
        if crossings >= gate {
            let (at, turn) = sw.fold(cell);
            let material = sw.material(at);
            if material != 0 {
                let entered = Face::new(axis, step[axis] > 0).opposite();
                let hit = Outcome {
                    hit: true,
                    cell: at,
                    face: turn.face(entered),
                    material,
                    t: t as f32,
                    steps,
                    crossings,
                };
                return (hit, t);
            }
        }
    }
    (
        Outcome {
            steps,
            crossings,
            ..MISS
        },
        f64::INFINITY,
    )
}

/// The engine's cursor.
pub fn engine(sw: &ScrewWorld, origin: Vec3, dir: Vec3, gate: u32, max_steps: u32) -> Outcome {
    let mut cur = Cursor::new(&sw.world, Ray::new(sw.room, origin, dir));
    let mut steps = 0;
    while steps < max_steps {
        let Moved::Entered { through, .. } = cur.advance() else {
            unreachable!("every face of the cube is glued");
        };
        steps += 1;
        if cur.crossings() >= gate && cur.material() != 0 {
            return Outcome {
                hit: true,
                cell: cur.cell().at,
                face: through,
                material: cur.material(),
                t: cur.t(),
                steps,
                crossings: cur.crossings(),
            };
        }
    }
    Outcome {
        steps,
        crossings: cur.crossings(),
        ..MISS
    }
}

/// Crossing by permutation, written against the engine's public API: at a
/// seam the voxel goes through the seam's integer map and the stepping state
/// is re-read in the new frame.
pub fn permuting<S: Stepper>(
    sw: &ScrewWorld,
    origin: Vec3,
    dir: Vec3,
    gate: u32,
    max_steps: u32,
) -> Outcome {
    let world = &sw.world;
    let room = world.room(sw.room);
    let mut cell = origin.floor();
    let mut s = S::new(origin, dir, cell);
    let (mut steps, mut crossings) = (0, 0);
    while steps < max_steps {
        let axis = s.next_axis();
        let face = Face::new(axis, s.step(axis) > 0);
        let t = s.advance(axis);
        let through;
        let here = Cell {
            room: sw.room,
            at: cell,
        };
        if let Some(id) = world.seam_at(here, face) {
            let map = world.seam(id).map;
            cell = map.cell(cell + face.normal());
            s = s.turned(map.rot);
            crossings += 1;
            through = map.rot.face(face).opposite();
        } else {
            cell[axis] += s.step(axis);
            through = face.opposite();
        }
        steps += 1;
        if crossings >= gate {
            let material = room.material(cell);
            if material != 0 {
                return Outcome {
                    hit: true,
                    cell,
                    face: through,
                    material,
                    t,
                    steps,
                    crossings,
                };
            }
        }
    }
    Outcome {
        steps,
        crossings,
        ..MISS
    }
}

/// How [`recompute`] finds the voxel a ray is in after a seam.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CellFrom {
    /// The voxel index is carried through the seam's integer map.
    Lattice,
    /// The voxel is the floor of the transformed position.
    Position,
}

/// What [`recompute`] measures distance from after a seam.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// The crossing point becomes the new origin and the distance restarts,
    /// with the distance so far kept as a sum. boxspace works this way, cube
    /// by cube.
    CrossingPoint,
    /// The ray's origin is transformed and one running distance is kept.
    /// fishpye works this way.
    Origin,
}

/// A variant of crossing by recomputation.
#[derive(Copy, Clone, Debug)]
pub struct Recompute {
    pub anchor: Anchor,
    pub cell_from: CellFrom,
    /// The point is taken this far beyond the face, to get it off the face.
    /// With [`Anchor::CrossingPoint`] the distance is counted.
    pub nudge: f32,
}

impl Recompute {
    /// The voxel is carried by the seam's integer map and the arithmetic
    /// restarts at the crossing point. boxspace carries its cube index this
    /// way; this is the most careful form of recomputation.
    pub const CAREFUL: Recompute = Recompute {
        anchor: Anchor::CrossingPoint,
        cell_from: CellFrom::Lattice,
        nudge: 0.0,
    };

    /// fishpye's crossing (`raytrace.cl` at commit 8e9cbef): the origin and
    /// direction are transformed, the running distance is kept, the voxel is
    /// the truncated point at that distance, and step, tDelta and tMax are
    /// reset from them.
    pub const FISHPYE: Recompute = Recompute {
        anchor: Anchor::Origin,
        cell_from: CellFrom::Position,
        nudge: 0.0,
    };

    /// The crossing point is moved `nudge` along the ray and the voxel is
    /// the floor of the result. noeuclid (`Shaders/Pass1.frag` at commit
    /// b2ab611) takes every voxel as the floor of a point moved 0.01 along
    /// the ray; it is not a DDA, so this stands for its rule and is not a
    /// copy of its loop.
    pub const fn nudged(nudge: f32) -> Recompute {
        Recompute {
            anchor: Anchor::CrossingPoint,
            cell_from: CellFrom::Position,
            nudge,
        }
    }
}

/// Crossing by recomputation: at a seam the crossing point and the direction
/// are transformed into the next room's frame, and step, tDelta and tMax are
/// worked out again from them.
pub fn recompute(
    sw: &ScrewWorld,
    cfg: Recompute,
    origin: Vec3,
    dir: Vec3,
    gate: u32,
    max_steps: u32,
) -> Outcome {
    let world = &sw.world;
    let room = world.room(sw.room);
    let top = sw.n - 1;
    let (mut o, mut d) = (origin, dir);
    let mut cell = origin.floor();
    let mut s = Running::new(o, d, cell);
    // Distance to the start of the current segment.
    let mut base = 0.0f32;
    let (mut steps, mut crossings) = (0, 0);
    while steps < max_steps {
        let axis = s.next_axis();
        let face = Face::new(axis, s.step(axis) > 0);
        let local = s.advance(axis);
        // Distance at which this face is crossed.
        let t = base + local;
        let through;
        let here = Cell {
            room: sw.room,
            at: cell,
        };
        if let Some(id) = world.seam_at(here, face) {
            let map = world.seam(id).map;
            let reach = local + cfg.nudge;
            // The point just past the face, in the next room's frame.
            let p = match cfg.anchor {
                Anchor::CrossingPoint => {
                    let p = map.point(o + d * reach);
                    o = p;
                    base += reach;
                    p
                }
                Anchor::Origin => {
                    o = map.point(o);
                    o + map.dir(d) * reach
                }
            };
            cell = match cfg.cell_from {
                CellFrom::Lattice => map.cell(cell + face.normal()),
                // A point that rounding or the nudge has put outside the
                // room is pulled back to the nearest voxel in it. Outside
                // has no meaning in this world, and the pull favours the
                // method: it repairs a point that fell just short of the
                // face.
                CellFrom::Position => p.floor().max(IVec3::ZERO).min(ivec3(top, top, top)),
            };
            d = map.dir(d);
            s = Running::new(o, d, cell);
            crossings += 1;
            through = map.rot.face(face).opposite();
        } else {
            cell[axis] += s.step(axis);
            through = face.opposite();
        }
        steps += 1;
        if crossings >= gate {
            let material = room.material(cell);
            if material != 0 {
                return Outcome {
                    hit: true,
                    cell,
                    face: through,
                    material,
                    t,
                    steps,
                    crossings,
                };
            }
        }
    }
    Outcome {
        steps,
        crossings,
        ..MISS
    }
}

/// A point or a direction in homogeneous coordinates, as HyperRogue's shader
/// holds them: `w` is 1 for a point and 0 for a direction.
type V4 = [f32; 4];
/// Row-major: `m[row][column]`.
type M4 = [[f32; 4]; 4];

fn mul(m: &M4, v: V4) -> V4 {
    let row = |r: usize| m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2] + m[r][3] * v[3];
    [row(0), row(1), row(2), row(3)]
}

fn dot(a: V4, b: V4) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

fn along(p: V4, t: V4, d: f32) -> V4 {
    [
        p[0] + t[0] * d,
        p[1] + t[1] * d,
        p[2] + t[2] * d,
        p[3] + t[3] * d,
    ]
}

/// The matrix that takes coordinates about the centre of one voxel to
/// coordinates about the centre of the voxel beyond `face`, and that voxel.
/// Away from a seam it is a shift of one voxel. At a seam it is the seam's
/// turn and the shift together. Its entries are 0, 1 and -1.
fn to_neighbour(sw: &ScrewWorld, cell: IVec3, face: Face) -> (M4, IVec3, Face, bool) {
    let here = Cell {
        room: sw.room,
        at: cell,
    };
    let n = face.normal();
    let (rot, next, seam) = match sw.world.seam_at(here, face) {
        Some(id) => {
            let map = sw.world.seam(id).map;
            (map.rot, map.cell(cell + n), true)
        }
        None => (Orientation::IDENTITY, cell + n, false),
    };
    let mut m = [[0.0f32; 4]; 4];
    for (j, row) in m.iter_mut().enumerate().take(3) {
        let (src, sign) = rot.source(j);
        row[src] = sign as f32;
        // The shift, turned: minus the face's normal, read in the new frame.
        row[3] = -(n[src] * sign) as f32;
    }
    m[3][3] = 1.0;
    (m, next, rot.face(face).opposite(), seam)
}

/// Crossing as HyperRogue's ray shader does it (`raycaster.cpp`, in its
/// Euclidean branch, read at the repository's commit 54a3125).
///
/// The ray is a position and a tangent in the coordinates of the cell it is
/// in, with the cell's centre at the origin. Every wall of every cell is
/// crossed the same way, seam or not. For each of the six walls the shader
/// takes the matrix `m` to the neighbour's coordinates and finds the distance
/// `d` at which the ray is as far from this centre as from the neighbour's.
/// It moves the position to the nearest wall, and multiplies the position and
/// the tangent by that wall's matrix. The expressions below are the shader's,
/// in its order. Distance is its `go`, a sum of the wall-to-wall distances.
///
/// The shader declares its numbers `mediump`. Desktop OpenGL gives that 32
/// bits, which is what this runs in. A device that gives `mediump` 16 bits
/// would do worse and is not measured.
pub fn cellwise(sw: &ScrewWorld, origin: Vec3, dir: Vec3, gate: u32, max_steps: u32) -> Outcome {
    let room = sw.world.room(sw.room);
    let mut cell = origin.floor();
    let mut position: V4 = [0.0, 0.0, 0.0, 1.0];
    let mut tangent: V4 = [dir[0], dir[1], dir[2], 0.0];
    for i in 0..3 {
        position[i] = origin[i] - (cell[i] as f32 + 0.5);
    }
    let mut go = 0.0f32;
    let (mut steps, mut crossings) = (0, 0);
    while steps < max_steps {
        steps += 1;
        let mut dist = 100.0f32;
        let mut which = None;
        for face in Face::ALL {
            let wall = to_neighbour(sw, cell, face);
            let m = &wall.0;
            let deno = dot(position, tangent) - dot(mul(m, position), mul(m, tangent));
            if deno < 1e-6 && deno > -1e-6 {
                continue;
            }
            let d =
                (dot(mul(m, position), mul(m, position)) - dot(position, position)) / 2.0 / deno;
            if d < 0.0 {
                continue;
            }
            let next_position = along(position, tangent, d);
            if dot(next_position, tangent) < dot(mul(m, next_position), mul(m, tangent)) {
                continue;
            }
            if d < dist {
                dist = d;
                which = Some(wall);
            }
        }
        position = along(position, tangent, dist);
        go += dist;
        let Some((m, next, through, seam)) = which else {
            continue;
        };
        if seam {
            crossings += 1;
        }
        if crossings >= gate {
            let material = room.material(next);
            if material != 0 {
                return Outcome {
                    hit: true,
                    cell: next,
                    face: through,
                    material,
                    t: go,
                    steps,
                    crossings,
                };
            }
        }
        position = mul(&m, position);
        tangent = mul(&m, tangent);
        cell = next;
    }
    Outcome {
        steps,
        crossings,
        ..MISS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The matrix baseline is a tracer: over short rays it stops on the
    /// surface the true ray stops on, at the true distance to within a few
    /// rounding errors.
    #[test]
    fn the_matrix_baseline_traces_short_rays_correctly() {
        let mut sw = ScrewWorld::new(8);
        sw.scatter(&mut Rng::new(7), 0.15);
        let mut rng = Rng::new(19);
        let mut same = 0;
        for k in 0..2000 {
            let origin = vec3(rng.unit() * 8.0, rng.unit() * 8.0, rng.unit() * 8.0);
            let dir = rng.direction();
            let (real, real_t) = truth(&sw, origin, dir, 2, 100_000);
            assert!(real.hit, "ray {k}");
            let got = cellwise(&sw, origin, dir, 2, 100_000);
            if got.hit && got.cell == real.cell && got.face == real.face {
                assert!((got.t as f64 - real_t).abs() < 1e-3, "ray {k}");
                assert_eq!(got.crossings, real.crossings, "ray {k}");
                same += 1;
            }
        }
        // A ray that grazes an edge may fall on either side of it.
        assert!(same > 1990, "{same}");
    }

    /// The claim the measurements rest on, kept true by the test suite: after
    /// hundreds of crossings a permuting trace is the uncut trace, bit for
    /// bit, in either form of stepping state.
    #[test]
    fn permuting_is_the_uncut_trace_after_many_crossings() {
        let mut sw = ScrewWorld::new(8);
        sw.scatter(&mut Rng::new(7), 0.15);
        let mut rng = Rng::new(11);
        let mut crossed = 0;
        for k in 0..600 {
            let origin = vec3(rng.unit() * 8.0, rng.unit() * 8.0, rng.unit() * 8.0);
            let dir = rng.direction();
            let gate = [3, 40, 300][k % 3];
            let limit = 100_000;

            let running = unrolled::<Running>(&sw, origin, dir, gate, limit);
            assert!(running.hit, "ray {k}");
            assert!(permuting::<Running>(&sw, origin, dir, gate, limit).identical(&running));

            let counted = unrolled::<Counted>(&sw, origin, dir, gate, limit);
            assert!(
                engine(&sw, origin, dir, gate, limit).identical(&counted),
                "ray {k}"
            );
            assert!(permuting::<Counted>(&sw, origin, dir, gate, limit).identical(&counted));
            crossed += running.crossings;
        }
        assert!(crossed > 60_000, "{crossed}");
    }

    /// Folding agrees with walking: the engine's own lattice walk from a
    /// voxel reaches the voxel that folding the far end gives.
    #[test]
    fn fold_agrees_with_the_engine_walk() {
        let sw = ScrewWorld::new(5);
        let mut rng = Rng::new(3);
        for _ in 0..500 {
            let start = ivec3(rng.below(0, 5), rng.below(0, 5), rng.below(0, 5));
            let delta = ivec3(rng.below(-23, 24), rng.below(-23, 24), rng.below(-23, 24));
            let here = Cell {
                room: sw.room,
                at: start,
            };
            let (cell, turn) = sw.world.walk(here, delta).expect("nothing leads out");
            assert_eq!(
                (cell.at, turn),
                sw.fold(start + delta),
                "{start:?} + {delta:?}"
            );
        }
    }
}
