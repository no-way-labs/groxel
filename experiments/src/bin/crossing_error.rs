//! How far each way of crossing a seam is from the unrolled world, as a
//! function of how many seams the ray crossed before it hit something.
//!
//! ```text
//! cargo run --release -p groxel-experiments --bin crossing_error -- [out.csv [world-seed ray-seed]]
//! ```
//!
//! The seeds are hexadecimal. Without them the run uses the seeds the
//! experiment was designed on; a run with other seeds confirms it.
//!
//! Each ray passes through its first `c` seams unobstructed and then stops at
//! the first solid voxel. The same ray is traced in the unrolled world, which
//! has no seams, twice: in single precision (what an engine without seams
//! would compute) and in double precision (where the ray really goes). Each
//! way of crossing is compared with both.

use groxel::{Vec3, vec3};
use groxel_experiments::{
    Counted, Outcome, Recompute, Rng, Running, ScrewWorld, cellwise, engine, permuting, recompute,
    truth, unrolled,
};
use std::fmt::Write as _;

const SIDE: i32 = 8;
const DENSITY: f32 = 0.15;
const WORLD_SEED: u64 = 0x6A09_E667_F3BC_C908;
const RAY_SEED: u64 = 0xBB67_AE85_84CA_A73B;
const RAYS: usize = 20_000;
const CROSSINGS: [u32; 11] = [0, 1, 2, 5, 10, 20, 50, 100, 200, 500, 1000];
/// A hit on another surface whose distance is within this of the true one is
/// a ray that passed beside a voxel edge and fell on the other side of it.
/// Further off, the ray went through something or stopped short.
const BESIDE: f64 = 0.05;

type Trace = fn(&ScrewWorld, Vec3, Vec3, u32, u32) -> Outcome;

struct Method {
    name: &'static str,
    run: Trace,
    /// The uncut trace this method should reproduce bit for bit.
    uncut: Trace,
}

const METHODS: [Method; 10] = [
    Method {
        name: "uncut",
        run: unrolled::<Counted>,
        uncut: unrolled::<Counted>,
    },
    Method {
        name: "engine",
        run: engine,
        uncut: unrolled::<Counted>,
    },
    Method {
        name: "perm-count",
        run: permuting::<Counted>,
        uncut: unrolled::<Counted>,
    },
    Method {
        name: "uncut-run",
        run: unrolled::<Running>,
        uncut: unrolled::<Running>,
    },
    Method {
        name: "perm-run",
        run: permuting::<Running>,
        uncut: unrolled::<Running>,
    },
    Method {
        name: "recompute",
        run: |w, o, d, g, m| recompute(w, Recompute::CAREFUL, o, d, g, m),
        uncut: unrolled::<Counted>,
    },
    Method {
        name: "fishpye",
        run: |w, o, d, g, m| recompute(w, Recompute::FISHPYE, o, d, g, m),
        uncut: unrolled::<Counted>,
    },
    Method {
        name: "floor-1e-3",
        run: |w, o, d, g, m| recompute(w, Recompute::nudged(1e-3), o, d, g, m),
        uncut: unrolled::<Counted>,
    },
    Method {
        name: "floor-1e-2",
        run: |w, o, d, g, m| recompute(w, Recompute::nudged(1e-2), o, d, g, m),
        uncut: unrolled::<Counted>,
    },
    Method {
        name: "cell-matrix",
        run: cellwise,
        uncut: unrolled::<Counted>,
    },
];

#[derive(Default, Clone)]
struct Tally {
    rays: usize,
    /// The same result as the uncut trace, bit for bit: voxel, face,
    /// material, distance and number of voxels visited.
    as_uncut: usize,
    /// Against the true ray: the same voxel and face.
    same_surface: usize,
    /// Against the true ray: another voxel or face, at nearly the same
    /// distance.
    beside: usize,
    /// Against the true ray: another voxel or face, at another distance.
    wrong: usize,
    /// No hit within the step limit.
    lost: usize,
    /// Distance from the true hit, for rays on the same surface.
    errors: Vec<f64>,
}

impl Tally {
    fn add(&mut self, uncut: &Outcome, real: &Outcome, real_t: f64, got: &Outcome) {
        self.rays += 1;
        if got.identical(uncut) {
            self.as_uncut += 1;
        }
        if !got.hit {
            self.lost += 1;
            return;
        }
        let err = (got.t as f64 - real_t).abs();
        if got.cell == real.cell && got.face == real.face {
            self.same_surface += 1;
            self.errors.push(err);
        } else if err <= BESIDE {
            self.beside += 1;
        } else {
            self.wrong += 1;
        }
    }

    fn quantile(&mut self, q: f64) -> f64 {
        if self.errors.is_empty() {
            return f64::NAN;
        }
        self.errors.sort_by(f64::total_cmp);
        let i = ((self.errors.len() - 1) as f64 * q).round() as usize;
        self.errors[i]
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned();
    let seed = |i: usize, default: u64| match args.get(i) {
        Some(text) => u64::from_str_radix(text.trim_start_matches("0x"), 16)
            .expect("a seed is a hexadecimal number"),
        None => default,
    };
    let (world_seed, ray_seed) = (seed(2, WORLD_SEED), seed(3, RAY_SEED));

    let mut sw = ScrewWorld::new(SIDE);
    sw.scatter(&mut Rng::new(world_seed), DENSITY);

    let mut csv = String::from(
        "method,crossings,rays,as_uncut,same_surface,beside,wrong,lost,\
         err_median,err_p99,err_max,mean_distance\n",
    );
    println!(
        "quarter-turn space, side {SIDE}, density {DENSITY}, {RAYS} rays per row, \
         world seed {world_seed:#x}, ray seed {ray_seed:#x}"
    );
    println!(
        "{:<11} {:>5} {:>8} | {:>8} {:>7} {:>6} {:>5} {:>10} {:>10} {:>10}",
        "method", "cross", "as uncut", "surface", "beside", "wrong", "lost", "median", "p99", "max"
    );

    for &gate in &CROSSINGS {
        // The same rays for every method.
        let mut rng = Rng::new(ray_seed ^ gate as u64);
        let mut tallies = vec![Tally::default(); METHODS.len()];
        let mut distance = 0.0f64;
        let mut used = 0;
        while used < RAYS {
            let origin = vec3(
                rng.unit() * SIDE as f32,
                rng.unit() * SIDE as f32,
                rng.unit() * SIDE as f32,
            );
            let dir = rng.direction();
            let (real, real_t) = truth(&sw, origin, dir, gate, 1_000_000);
            // With no gate a ray may hit before any crossing; keep the first
            // row to rays that crossed nothing.
            if !real.hit || (gate == 0 && real.crossings != 0) {
                continue;
            }
            used += 1;
            distance += real_t;
            let limit = real.steps + 10_000;
            for (method, tally) in METHODS.iter().zip(&mut tallies) {
                let uncut = (method.uncut)(&sw, origin, dir, gate, limit);
                let got = (method.run)(&sw, origin, dir, gate, limit);
                tally.add(&uncut, &real, real_t, &got);
            }
        }
        for (method, tally) in METHODS.iter().zip(&mut tallies) {
            let (median, p99, max) = (
                tally.quantile(0.5),
                tally.quantile(0.99),
                tally.quantile(1.0),
            );
            println!(
                "{:<11} {:>5} {:>8} | {:>8} {:>7} {:>6} {:>5} {:>10.2e} {:>10.2e} {:>10.2e}",
                method.name,
                gate,
                tally.as_uncut,
                tally.same_surface,
                tally.beside,
                tally.wrong,
                tally.lost,
                median,
                p99,
                max
            );
            writeln!(
                csv,
                "{},{},{},{},{},{},{},{},{:e},{:e},{:e},{:.3}",
                method.name,
                gate,
                tally.rays,
                tally.as_uncut,
                tally.same_surface,
                tally.beside,
                tally.wrong,
                tally.lost,
                median,
                p99,
                max,
                distance / used as f64
            )
            .unwrap();
        }
        println!();
    }

    if let Some(path) = out {
        std::fs::write(&path, csv).expect("write the csv");
        println!("wrote {path}");
    }
}
