//! Whole pictures drawn with each way of crossing a seam, compared pixel by
//! pixel with the picture the uncut trace draws.
//!
//! ```text
//! cargo run --release -p groxel-experiments --bin frames -- [out.csv [world-seed camera-seed [png-folder]]]
//! ```
//!
//! The world is the quarter-turn space with a few solid voxels in it, so a
//! view is the same few voxels repeated into the distance through its seams.
//! Each picture is one ray a pixel. A pixel's colour comes from the material
//! and the face the ray stopped on, faded with distance, and is rounded to 8
//! bits a channel as a display would have it. Shadows, textures and anything
//! else that reads the hit point are left out, so distance reaches the
//! picture only through the fade.
//!
//! The seeds are hexadecimal. With a folder, the first view is written there
//! as PNG files: the uncut picture, each method's picture, and for each
//! method the uncut picture dimmed with the differing pixels marked in red.

use groxel::{Face, Vec3, vec3};
use groxel_experiments::{
    Counted, Outcome, Recompute, Rng, Running, ScrewWorld, cellwise, engine, permuting, recompute,
    unrolled,
};
use groxel_render::write_png;
use std::fmt::Write as _;

const SIDE: i32 = 8;
const DENSITY: f32 = 0.02;
const WORLD_SEED: u64 = 0x510E_527F_ADE6_82D1;
const CAMERA_SEED: u64 = 0x9B05_688C_2B3E_6C1F;
const VIEWS: usize = 8;
const WIDTH: usize = 640;
const HEIGHT: usize = 360;
/// Half the width of the view, as a tangent: about 70 degrees across.
const HALF_WIDTH: f32 = 0.7;
const MAX_STEPS: u32 = 4_000;
/// The distance, in voxels, over which a surface fades to 1/e.
const FADE: f32 = 40.0;
const SKY: [f32; 3] = [0.62, 0.72, 0.84];

type Trace = fn(&ScrewWorld, Vec3, Vec3, u32, u32) -> Outcome;

struct Method {
    name: &'static str,
    run: Trace,
    /// The uncut trace whose picture this method should reproduce.
    uncut: Trace,
}

const METHODS: [Method; 7] = [
    Method {
        name: "engine",
        run: engine,
        uncut: unrolled::<Counted>,
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

/// The colour of a pixel, as `0x00BBGGRR`.
fn shade(o: &Outcome) -> u32 {
    let mut rgb = SKY;
    if o.hit {
        let m = o.material as u32;
        let base = [
            0.30 + 0.65 * (m * 37 % 97) as f32 / 96.0,
            0.30 + 0.65 * (m * 59 % 89) as f32 / 88.0,
            0.30 + 0.65 * (m * 83 % 101) as f32 / 100.0,
        ];
        let side = Face::ALL.iter().position(|f| *f == o.face).unwrap();
        let lit = [0.90, 0.60, 1.00, 0.45, 0.78, 0.68][side];
        let fade = (-o.t / FADE).exp();
        for i in 0..3 {
            rgb[i] = SKY[i] + (base[i] * lit - SKY[i]) * fade;
        }
    }
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    byte(rgb[0]) | byte(rgb[1]) << 8 | byte(rgb[2]) << 16
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / len, v[1] / len, v[2] / len]
}

struct Camera {
    origin: Vec3,
    fwd: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
}

impl Camera {
    /// Somewhere in an empty voxel, looking any way.
    fn random(sw: &ScrewWorld, rng: &mut Rng) -> Camera {
        let n = sw.n as f32;
        let origin = loop {
            let p = vec3(rng.unit() * n, rng.unit() * n, rng.unit() * n);
            if sw.material(p.floor()) == 0 {
                break p;
            }
        };
        let d = rng.direction();
        let fwd = [d[0], d[1], d[2]];
        let hint = if fwd[1].abs() < 0.9 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let right = unit(cross(fwd, hint));
        let up = cross(right, fwd);
        Camera {
            origin,
            fwd,
            right,
            up,
        }
    }

    /// The ray through the middle of a pixel.
    fn ray(&self, x: usize, y: usize) -> Vec3 {
        let u = ((x as f32 + 0.5) / WIDTH as f32 * 2.0 - 1.0) * HALF_WIDTH;
        let v = (1.0 - (y as f32 + 0.5) / HEIGHT as f32 * 2.0) * HALF_WIDTH * HEIGHT as f32
            / WIDTH as f32;
        let d = unit([
            self.fwd[0] + self.right[0] * u + self.up[0] * v,
            self.fwd[1] + self.right[1] * u + self.up[1] * v,
            self.fwd[2] + self.right[2] * u + self.up[2] * v,
        ]);
        vec3(d[0], d[1], d[2])
    }
}

#[derive(Default, Clone)]
struct Tally {
    pixels: usize,
    /// Pixels whose 8-bit colour is not the uncut picture's.
    differ: usize,
    /// Of those, pixels whose ray stopped on another voxel or face, or
    /// stopped where the uncut ray did not, or the reverse.
    surface: usize,
    /// Rays whose result is not the uncut trace's, bit for bit.
    rays: usize,
    /// The most pixels that differ in one view.
    worst_view: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned();
    let seed = |i: usize, default: u64| match args.get(i) {
        Some(text) => u64::from_str_radix(text.trim_start_matches("0x"), 16)
            .expect("a seed is a hexadecimal number"),
        None => default,
    };
    let (world_seed, camera_seed) = (seed(2, WORLD_SEED), seed(3, CAMERA_SEED));
    let folder = args.get(4).cloned();

    let mut sw = ScrewWorld::new(SIDE);
    sw.scatter(&mut Rng::new(world_seed), DENSITY);
    let mut rng = Rng::new(camera_seed);

    println!(
        "quarter-turn space, side {SIDE}, density {DENSITY}, {VIEWS} views of {WIDTH} x {HEIGHT}, \
         world seed {world_seed:#x}, camera seed {camera_seed:#x}"
    );
    let mut tallies = vec![Tally::default(); METHODS.len()];
    let (mut crossed, mut longest, mut sky) = (0u64, 0u32, 0usize);
    for view in 0..VIEWS {
        let camera = Camera::random(&sw, &mut rng);
        let mut differ = vec![0usize; METHODS.len()];
        let mut pictures = vec![vec![0u32; WIDTH * HEIGHT]; METHODS.len()];
        let mut marked = vec![vec![0u32; WIDTH * HEIGHT]; METHODS.len()];
        let mut uncut_picture = vec![0u32; WIDTH * HEIGHT];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let dir = camera.ray(x, y);
                let at = y * WIDTH + x;
                for (i, method) in METHODS.iter().enumerate() {
                    let want = (method.uncut)(&sw, camera.origin, dir, 0, MAX_STEPS);
                    let got = (method.run)(&sw, camera.origin, dir, 0, MAX_STEPS);
                    let (a, b) = (shade(&want), shade(&got));
                    if i == 0 {
                        uncut_picture[at] = a;
                        crossed += want.crossings as u64;
                        longest = longest.max(want.crossings);
                        sky += !want.hit as usize;
                    }
                    pictures[i][at] = b;
                    let tally = &mut tallies[i];
                    tally.pixels += 1;
                    tally.rays += !got.identical(&want) as usize;
                    if a != b {
                        tally.differ += 1;
                        differ[i] += 1;
                        marked[i][at] = 1;
                        let same = got.hit == want.hit
                            && (!got.hit || (got.cell == want.cell && got.face == want.face));
                        tally.surface += !same as usize;
                    }
                }
            }
        }
        for (tally, count) in tallies.iter_mut().zip(&differ) {
            tally.worst_view = tally.worst_view.max(*count);
        }
        if let (0, Some(folder)) = (view, &folder) {
            std::fs::create_dir_all(folder).expect("make the folder");
            let save = |name: &str, pixels: &[u32]| {
                write_png(
                    format!("{folder}/{name}.png"),
                    WIDTH as u32,
                    HEIGHT as u32,
                    pixels,
                )
                .expect("write a picture");
            };
            save("uncut", &uncut_picture);
            for (i, method) in METHODS.iter().enumerate() {
                save(method.name, &pictures[i]);
                // The uncut picture at a third of its brightness, with each
                // differing pixel and its eight neighbours in red.
                let mut dim: Vec<u32> = uncut_picture
                    .iter()
                    .map(|p| {
                        let third = |shift: u32| ((p >> shift & 0xFF) / 3) << shift;
                        third(0) | third(8) | third(16)
                    })
                    .collect();
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        if marked[i][y * WIDTH + x] == 0 {
                            continue;
                        }
                        for (dx, dy) in
                            (-1i32..=1).flat_map(|dx| (-1i32..=1).map(move |dy| (dx, dy)))
                        {
                            let (px, py) = (x as i32 + dx, y as i32 + dy);
                            if (0..WIDTH as i32).contains(&px) && (0..HEIGHT as i32).contains(&py) {
                                dim[py as usize * WIDTH + px as usize] = 0x0000_00FF;
                            }
                        }
                    }
                }
                save(&format!("{}_marked", method.name), &dim);
            }
            println!("wrote the first view to {folder}");
        }
    }

    let pixels = VIEWS * WIDTH * HEIGHT;
    println!(
        "{pixels} pixels; a ray crosses {:.1} seams on average and at most {longest}; \
         {sky} rays stop nowhere within {MAX_STEPS} steps",
        crossed as f64 / pixels as f64
    );
    println!(
        "{:<11} {:>9} {:>9} {:>10} {:>11}",
        "method", "differ", "surface", "worst view", "rays differ"
    );
    let mut csv = String::from("method,pixels,differ,other_surface,worst_view,rays_differ\n");
    for (method, tally) in METHODS.iter().zip(&tallies) {
        println!(
            "{:<11} {:>9} {:>9} {:>10} {:>11}",
            method.name, tally.differ, tally.surface, tally.worst_view, tally.rays
        );
        writeln!(
            csv,
            "{},{},{},{},{},{}",
            method.name, tally.pixels, tally.differ, tally.surface, tally.worst_view, tally.rays
        )
        .unwrap();
    }
    writeln!(
        csv,
        "# mean_crossings,{:.3},max_crossings,{longest},no_hit,{sky}",
        crossed as f64 / pixels as f64
    )
    .unwrap();
    if let Some(path) = out {
        std::fs::write(&path, csv).expect("write the csv");
        println!("wrote {path}");
    }
}
