//! The Hall, through the core alone: build it, walk it, and look.
//!
//!   cargo run --release --example hall -- out
//!
//! Writes `out/hall-0.ppm` to `out/hall-3.ppm`: the view after 0, 1, 2 and 3
//! lengths of the corridor. The walker feels upright in every one. The
//! shading is the least that makes a picture; the renderer is a later crate.

use groxel::{Face, HitKind, Pose, Ray, TraceOpts, Vec3, World, trace, vec3};
use std::io::Write;

#[path = "../tests/common/mod.rs"]
mod common;
use common::hall;

const W: usize = 640;
const H: usize = 360;

fn colour(material: u8, at: groxel::IVec3) -> [f32; 3] {
    match material {
        hall::CHECKER => {
            let dark = (at[0].div_euclid(4) + at[1].div_euclid(4) + at[2].div_euclid(4))
                .rem_euclid(2)
                == 1;
            if dark {
                [0.05, 0.05, 0.07]
            } else {
                [0.70, 0.68, 0.60]
            }
        }
        hall::WOOD => [0.22, 0.11, 0.04],
        hall::PLASTER => [0.70, 0.64, 0.52],
        hall::BRICK => [0.40, 0.19, 0.12],
        hall::CARPET => [0.42, 0.02, 0.03],
        hall::IRON => [0.03, 0.03, 0.04],
        hall::LAMP => [2.2, 1.5, 0.5],
        _ => [0.5, 0.5, 0.5],
    }
}

fn shade(world: &World, ray: Ray) -> [f32; 3] {
    let hit = trace(
        world,
        ray,
        &TraceOpts {
            max_t: 340.0,
            ..Default::default()
        },
    );
    let dusk = [0.012, 0.010, 0.016];
    match hit.kind {
        HitKind::Sky => [0.55, 0.70, 0.95],
        HitKind::Far => dusk,
        HitKind::Solid => {
            let mut c = colour(hit.material, hit.cell.at);
            if hit.material != hall::LAMP {
                // Lit from the room's own +y, in the room's own frame: so a
                // surface looks the same whichever way round you meet it.
                let light = match hit.face {
                    Face::PosY => 1.0,
                    Face::NegY => 0.45,
                    Face::PosX => 0.80,
                    Face::NegX => 0.62,
                    _ => 0.70,
                };
                // A dark line round each voxel face, to show the grid.
                let p = hit.point();
                let (u, v) = ((hit.face.axis() + 1) % 3, (hit.face.axis() + 2) % 3);
                let edge = |x: f32| (x - x.floor()).min(1.0 - (x - x.floor()));
                let line = if edge(p[u]).min(edge(p[v])) < 0.03 + hit.t * 0.0006 {
                    0.82
                } else {
                    1.0
                };
                for k in &mut c {
                    *k *= light * line;
                }
            }
            let fog = 1.0 - (-0.0075 * hit.t).exp();
            [
                c[0] + (dusk[0] - c[0]) * fog,
                c[1] + (dusk[1] - c[1]) * fog,
                c[2] + (dusk[2] - c[2]) * fog,
            ]
        }
    }
}

fn render(world: &World, eye: &Pose, yaw: f32, pitch: f32) -> Vec<u8> {
    // The camera's own axes: x right, y up, z forward. The pose's frame says
    // how those read in the room.
    let (sy, cy, sp, cp) = (yaw.sin(), yaw.cos(), pitch.sin(), pitch.cos());
    let forward = eye.to_room(vec3(sy * cp, sp, cy * cp));
    let right = eye.to_room(vec3(-cy, 0.0, sy));
    let up = right.cross(forward);
    let tan_x = (80.0f32.to_radians() / 2.0).tan();
    let tan_y = tan_x * H as f32 / W as f32;

    let mut rgb = Vec::with_capacity(W * H * 3);
    for py in 0..H {
        for px in 0..W {
            let kx = ((px as f32 + 0.5) / W as f32 * 2.0 - 1.0) * tan_x;
            let ky = (1.0 - (py as f32 + 0.5) / H as f32 * 2.0) * tan_y;
            let dir: Vec3 = (forward + right * kx + up * ky).normalized();
            let c = shade(
                world,
                Ray {
                    start: eye.cell,
                    origin: eye.pos,
                    dir,
                },
            );
            for k in c {
                rgb.push((k.max(0.0).sqrt().min(1.0) * 255.0) as u8);
            }
        }
    }
    rgb
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "out".into());
    std::fs::create_dir_all(&dir)?;

    let mut world = World::new();
    let id = hall::build(&mut world)?;
    let mut eye = Pose::new(id, vec3(10.5, 7.0, 4.5));

    for length in 0..4 {
        let started = std::time::Instant::now();
        let rgb = render(&world, &eye, 0.0, -0.06);
        let path = format!("{dir}/hall-{length}.ppm");
        let mut file = std::io::BufWriter::new(std::fs::File::create(&path)?);
        write!(file, "P6\n{W} {H}\n255\n")?;
        file.write_all(&rgb)?;
        println!(
            "{path}: up is {:?} in the room, frame {:?}, {:.0} ms",
            eye.up(),
            eye.frame,
            started.elapsed().as_secs_f32() * 1000.0
        );
        eye.advance_local(&world, vec3(0.0, 0.0, hall::LEN as f32));
    }
    Ok(())
}
