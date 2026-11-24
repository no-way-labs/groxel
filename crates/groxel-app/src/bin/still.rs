//! A headless camera: one still of the demo worlds, written as a PNG.
//!
//! ```text
//! still <spot> <out.png> [key=value ...]
//! ```
//!
//! The spots are `gallery`, `well`, `tower`, `overlook` and `hall`.
//!
//! Keys: `w h ss fov` (image), `dx dy dz yaw pitch` (camera, degrees),
//! `t` (seconds of world time before the shot), `me` (0 hides your own
//! figure), `walk` (voxels to walk forward first), `fly`, and
//! `shadows ao ink levels threads`. [`Shot::parse`] has the full table.
//!
//! The five pictures in the README are these shots:
//!
//! ```text
//! still gallery  gallery.png w=1280 h=720 t=3
//! still gallery  mirror.png  w=1280 h=720 t=3 yaw=150 dx=-6 dz=6
//! still well     well.png    w=1280 h=720 t=0.5 dx=22 dz=0.4 yaw=20 pitch=48
//! still overlook tower.png   w=1280 h=720 t=4 dy=-20
//! still hall     hall.png    w=1280 h=720 t=3
//! ```

use groxel_app::Demo;
use groxel_app::shot::Shot;
use groxel_render::write_png;
use std::process::ExitCode;
use std::time::Instant;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [name, path, rest @ ..] = args.as_slice() else {
        eprintln!("usage: still <spot> <out.png> [key=value ...]");
        eprintln!("keys: {}", Shot::KEYS);
        return ExitCode::FAILURE;
    };

    let started = Instant::now();
    let mut demo = match Demo::new() {
        Ok(demo) => demo,
        Err(why) => {
            eprintln!("the worlds do not fit together: {why}");
            return ExitCode::FAILURE;
        }
    };
    let built = started.elapsed();

    let Some(spot) = demo.spot(name) else {
        let names: Vec<&str> = demo.spots.iter().map(|spot| spot.name).collect();
        eprintln!("no spot called '{name}'. Spots: {}", names.join(" "));
        return ExitCode::FAILURE;
    };
    let mut shot = Shot::default();
    if let Err(why) = shot.parse(rest) {
        eprintln!("{why}");
        return ExitCode::FAILURE;
    }

    let start = shot.start(&spot);
    let room = demo.world.room(start.room);
    if !room.contains(start.anchor().at) {
        let d = room.dims();
        eprintln!(
            "dx, dy and dz put the camera outside the {} ({} x {} x {} voxels)",
            room.name(),
            d[0],
            d[1],
            d[2]
        );
        return ExitCode::FAILURE;
    }

    let body = shot.stage(&mut demo, &spot);
    let started = Instant::now();
    let pixels = shot.render(&demo, &body);
    let rendered = started.elapsed();

    let s = &shot.settings;
    if let Err(why) = write_png(path, s.width, s.height, &pixels) {
        eprintln!("cannot write {path}: {why}");
        return ExitCode::FAILURE;
    }
    println!(
        "wrote {path}: {} x {}, ss={}, rendered in {:.1} ms (worlds built in {:.1} ms)",
        s.width,
        s.height,
        s.samples.max(1),
        rendered.as_secs_f64() * 1e3,
        built.as_secs_f64() * 1e3,
    );
    ExitCode::SUCCESS
}
