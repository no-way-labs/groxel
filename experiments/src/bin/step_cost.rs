//! What a step costs, and what a seam crossing adds to it.
//!
//! ```text
//! cargo run --release -p groxel-experiments --bin step_cost -- [out.csv]
//! ```
//!
//! Part 1 times each way of crossing in empty quarter-turn spaces of several
//! sizes. A smaller cube means more of the steps are crossings, so the time
//! per step as a function of the crossing rate gives two numbers by least
//! squares: the cost of a plain step and the extra cost of a crossing.
//!
//! Part 2 prices the test of each face for a link, in a room with no seams.
//! It times one loop twice, with the test compiled in and with it left out,
//! and the engine's cursor beside them. The loop is the cursor's own, copied
//! here over a bare array of the engine's cells, so that the test is the only
//! difference between the two timings.
//!
//! One thread. Every figure is the fastest of several repeats.

use groxel::{Cursor, IVec3, Moved, Ray, Vec3, World, ivec3, vec3};
use groxel_experiments::{
    Counted, Outcome, Recompute, Rng, Running, ScrewWorld, engine, permuting, recompute,
};
use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;

const SIDES: [i32; 5] = [4, 8, 16, 32, 64];
const RAYS: usize = 4_000;
const STEPS: u32 = 1_000;
const REPEATS: usize = 9;
const RAY_SEED: u64 = 0x3C6E_F372_FE94_F82B;
const PLAIN_SIDE: i32 = 128;
const PLAIN_RAYS: usize = 40_000;
const PLAIN_REPEATS: usize = 31;

type Trace = fn(&ScrewWorld, Vec3, Vec3, u32, u32) -> Outcome;

const METHODS: [(&str, Trace); 6] = [
    ("engine", engine),
    ("perm-count", permuting::<Counted>),
    ("perm-run", permuting::<Running>),
    ("recompute", |w, o, d, g, m| {
        recompute(w, Recompute::CAREFUL, o, d, g, m)
    }),
    ("fishpye", |w, o, d, g, m| {
        recompute(w, Recompute::FISHPYE, o, d, g, m)
    }),
    ("floor-1e-2", |w, o, d, g, m| {
        recompute(w, Recompute::nudged(1e-2), o, d, g, m)
    }),
];

fn rays(side: i32, count: usize) -> Vec<(Vec3, Vec3)> {
    let mut rng = Rng::new(RAY_SEED);
    (0..count)
        .map(|_| {
            let origin = vec3(
                rng.unit() * side as f32,
                rng.unit() * side as f32,
                rng.unit() * side as f32,
            );
            (origin, rng.direction())
        })
        .collect()
}

/// Fit `y = a + b x` by least squares.
fn fit(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x, sy + y));
    let (mx, my) = (sx / n, sy / n);
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for (x, y) in points {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
    }
    let b = sxy / sxx;
    (my - b * mx, b)
}

fn crossing_cost(csv: &mut String) {
    println!("Part 1: empty quarter-turn spaces, {RAYS} rays of {STEPS} steps");
    println!(
        "{:<11} {:>5} {:>10} {:>9}",
        "method", "side", "cross/step", "ns/step"
    );
    let mut fits = Vec::new();
    for (name, run) in METHODS {
        let mut points = Vec::new();
        for side in SIDES {
            // Empty, so every ray runs its full number of steps, reading the
            // material of each voxel on the way.
            let sw = ScrewWorld::new(side);
            let rays = rays(side, RAYS);
            let mut best = f64::INFINITY;
            let (mut steps, mut crossings) = (0u64, 0u64);
            for _ in 0..REPEATS {
                (steps, crossings) = (0, 0);
                let start = Instant::now();
                for &(origin, dir) in &rays {
                    let out = run(black_box(&sw), black_box(origin), black_box(dir), 0, STEPS);
                    steps += out.steps as u64;
                    crossings += out.crossings as u64;
                }
                let ns = start.elapsed().as_nanos() as f64;
                best = best.min(ns / steps as f64);
            }
            let rate = crossings as f64 / steps as f64;
            println!("{name:<11} {side:>5} {rate:>10.4} {best:>9.2}");
            writeln!(csv, "crossing,{name},{side},{rate:.5},{best:.3}").unwrap();
            points.push((rate, best));
        }
        fits.push((name, fit(&points)));
        println!();
    }
    println!(
        "{:<11} {:>14} {:>18}",
        "method", "plain step ns", "extra per cross ns"
    );
    for (name, (plain, extra)) in fits {
        println!("{name:<11} {plain:>14.2} {extra:>18.2}");
        writeln!(csv, "fit,{name},,{plain:.3},{extra:.3}").unwrap();
    }
    println!();
}

/// The engine cursor's loop, copied out so that the test of a face for a
/// link can be compiled in or left out with nothing else changed. The cells
/// are the engine's: 16 bits, the material in the low byte and one link bit
/// for each face above it.
struct Walk<'a> {
    cells: &'a [u16],
    n: i32,
    cell: [i32; 3],
    index: usize,
    step: [i32; 3],
    t_max: [f32; 3],
    t_delta: [f32; 3],
    t_first: [f32; 3],
    count: [u32; 3],
    rank: [u8; 3],
    t: f32,
}

impl<'a> Walk<'a> {
    fn new(cells: &'a [u16], n: i32, origin: Vec3, dir: Vec3) -> Walk<'a> {
        let at: IVec3 = origin.floor();
        let mut step = [0i32; 3];
        let mut t_max = [f32::INFINITY; 3];
        let mut t_delta = [f32::INFINITY; 3];
        for i in 0..3 {
            let lo = at[i] as f32;
            let d = dir[i];
            if d > 0.0 {
                step[i] = 1;
                t_delta[i] = 1.0 / d;
                t_max[i] = ((lo + 1.0 - origin[i]) * t_delta[i]).max(0.0);
            } else if d < 0.0 {
                step[i] = -1;
                t_delta[i] = -1.0 / d;
                t_max[i] = ((origin[i] - lo) * t_delta[i]).max(0.0);
            }
        }
        let cell = [at[0], at[1], at[2]];
        Walk {
            cells,
            n,
            cell,
            index: (cell[0] + n * (cell[1] + n * cell[2])) as usize,
            step,
            t_max,
            t_delta,
            t_first: t_max,
            count: [0; 3],
            rank: [0, 1, 2],
            t: 0.0,
        }
    }

    /// The engine's rule, ties included.
    #[inline]
    fn next_axis(&self) -> usize {
        let (m, r) = (&self.t_max, &self.rank);
        let nearer = |a: usize, b: usize| {
            if m[a] < m[b] || (m[a] == m[b] && r[a] > r[b]) {
                a
            } else {
                b
            }
        };
        nearer(nearer(0, 1), 2)
    }

    /// Cross the next face; false when it leads out of the room. With
    /// `LINKS` the face's link bit is tested first, as the engine tests it.
    #[inline]
    fn advance<const LINKS: bool>(&mut self) -> bool {
        let axis = self.next_axis();
        let positive = self.step[axis] > 0;
        if LINKS {
            let bit = 1u16 << (8 + 2 * axis + positive as usize);
            if self.cells[self.index] & bit != 0 {
                return self.cross(axis);
            }
        }
        let next = self.cell[axis] + self.step[axis];
        if next < 0 || next >= self.n {
            return false;
        }
        self.t = self.t_max[axis];
        self.count[axis] += 1;
        self.t_max[axis] = self.t_first[axis] + self.count[axis] as f32 * self.t_delta[axis];
        self.cell[axis] = next;
        self.index = (self.cell[0] + self.n * (self.cell[1] + self.n * self.cell[2])) as usize;
        true
    }

    /// Stands for the engine's seam path. The room has no links, so it is
    /// never reached; it is here so that the test has something to guard.
    #[cold]
    #[inline(never)]
    fn cross(&mut self, axis: usize) -> bool {
        self.rank.swap(axis, (axis + 1) % 3);
        self.t = black_box(self.t);
        false
    }
}

fn walk<const LINKS: bool>(cells: &[u16], n: i32, origin: Vec3, dir: Vec3) -> (u32, u32) {
    let mut w = Walk::new(cells, n, origin, dir);
    let (mut steps, mut sum) = (0, 0u32);
    while w.advance::<LINKS>() {
        steps += 1;
        sum += (w.cells[w.index] & 0xFF) as u32;
    }
    black_box(w.t);
    (steps, sum)
}

fn through_cursor(world: &World, ray: Ray) -> (u32, u32) {
    let mut cur = Cursor::new(world, ray);
    let (mut steps, mut sum) = (0, 0u32);
    while cur.advance() != Moved::Left {
        steps += 1;
        sum += cur.material() as u32;
    }
    (steps, sum)
}

fn link_test_cost(csv: &mut String) {
    let n = PLAIN_SIDE;
    println!(
        "Part 2: a room of side {n} with no seams, {PLAIN_RAYS} rays from the middle, \
         {PLAIN_REPEATS} repeats in turn"
    );
    let mut world = World::new();
    let room = world.add_room("plain", ivec3(n, n, n));
    let cells = vec![0u16; (n * n * n) as usize];
    let mut rng = Rng::new(RAY_SEED);
    let mid = n as f32 / 2.0;
    let rays: Vec<(Vec3, Vec3)> = (0..PLAIN_RAYS)
        .map(|_| {
            let origin = vec3(
                mid + rng.unit() - 0.5,
                mid + rng.unit() - 0.5,
                mid + rng.unit() - 0.5,
            );
            (origin, rng.direction())
        })
        .collect();

    type Loop<'a> = &'a dyn Fn(Vec3, Vec3) -> (u32, u32);
    let without = |o, d| walk::<false>(black_box(&cells), n, o, d);
    let with = |o, d| walk::<true>(black_box(&cells), n, o, d);
    let cursor = |o, d| through_cursor(black_box(&world), Ray::new(room, o, d));
    let loops: [(&str, Loop); 3] = [
        ("loop without link test", &without),
        ("loop with link test", &with),
        ("engine cursor", &cursor),
    ];
    // The loops take turns, so that a slow moment of the machine falls on
    // all of them.
    let mut times = vec![Vec::new(); loops.len()];
    let mut steps = vec![0u64; loops.len()];
    for _ in 0..PLAIN_REPEATS {
        for (i, (_, f)) in loops.iter().enumerate() {
            let (mut count, mut sum) = (0u64, 0u64);
            let start = Instant::now();
            for &(origin, dir) in &rays {
                let (s, m) = f(black_box(origin), black_box(dir));
                count += s as u64;
                sum += m as u64;
            }
            let ns = start.elapsed().as_nanos() as f64;
            black_box(sum);
            times[i].push(ns / count as f64);
            steps[i] = count;
        }
    }
    println!(
        "{:<20} {:>9} {:>9} {:>9} {:>9}",
        "", "steps", "fastest", "median", "slowest"
    );
    for (i, (name, _)) in loops.iter().enumerate() {
        times[i].sort_by(f64::total_cmp);
        let (best, median, worst) = (
            times[i][0],
            times[i][PLAIN_REPEATS / 2],
            times[i][PLAIN_REPEATS - 1],
        );
        println!(
            "{name:<20} {:>9} {best:>9.3} {median:>9.3} {worst:>9.3}  ns/step",
            steps[i]
        );
        writeln!(csv, "plain,{name},{n},,{best:.3}").unwrap();
        writeln!(csv, "plain-median,{name},{n},,{median:.3}").unwrap();
    }
    assert_eq!(steps[0], steps[1], "the two loops walk the same voxels");
    assert_eq!(steps[0], steps[2], "and so does the cursor");
}

fn main() {
    let out = std::env::args().nth(1);
    let mut csv = String::from("part,method,side,crossings_per_step,ns_per_step\n");
    crossing_cost(&mut csv);
    link_test_cost(&mut csv);
    if let Some(path) = out {
        std::fs::write(&path, csv).expect("write the csv");
        println!("\nwrote {path}");
    }
}
