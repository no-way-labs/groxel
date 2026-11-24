//! The renderer, and the cut, turn and re-glue test applied to whole frames.

use groxel::{Cell, Orientation, Vec3, World, ivec3, vec3};
use groxel_light::Light;
use groxel_render::{Camera, Env, Frame, Material, Renderer, Settings};

#[path = "../../groxel/tests/common/mod.rs"]
mod common;
use common::{CutWorld, Rng};

const LAMP: u8 = 250;

fn glows(material: u8) -> bool {
    material == LAMP
}

#[test]
fn an_empty_room_renders_its_sky() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(8, 8, 8));
    let light = Light::flood(&w, glows, |_| false);
    let r = Renderer::new(Settings {
        width: 16,
        height: 8,
        threads: 2,
        ..Default::default()
    });
    let mut px = vec![0u32; 16 * 8];
    let camera = Camera {
        cell: Cell {
            room: id,
            at: ivec3(4, 4, 4),
        },
        pos: vec3(4.5, 4.5, 4.5),
        right: vec3(-1.0, 0.0, 0.0),
        up: vec3(0.0, 1.0, 0.0),
        fwd: vec3(0.0, 0.0, 1.0),
    };
    let frame = Frame {
        world: &w,
        overlay: None,
        light: &light,
        viewer: None,
    };
    r.render(&frame, &camera, &mut px);
    for v in px {
        assert!(v >> 24 == 0xFF && v & 0xFF_FFFF != 0);
    }
}

#[test]
fn a_lit_floor_is_brighter_than_a_shadowed_one() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(24, 16, 24));
    w.room_mut(id).fill(ivec3(0, 0, 0), ivec3(23, 0, 23), 1);
    w.room_mut(id).fill(ivec3(14, 8, 0), ivec3(23, 8, 23), 1); // a roof over half of it
    let light = Light::flood(&w, glows, |_| false);
    let mut r = Renderer::new(Settings {
        threads: 1,
        ink: 0.0,
        ..Default::default()
    });
    r.set_env(
        id,
        Env {
            sun_dir: [0.0, 1.0, 0.0],
            fog: 0.0,
            ..Default::default()
        },
    );
    let frame = Frame {
        world: &w,
        overlay: None,
        light: &light,
        viewer: None,
    };
    let down = vec3(0.0, -1.0, 0.0);
    let from = |x: i32| {
        let cell = Cell {
            room: id,
            at: ivec3(x, 4, 12),
        };
        r.shade(&frame, cell, cell.at.centre(), down, 0.001)
    };
    let (open, roofed) = (from(5), from(18));
    assert!(open[0] > roofed[0] * 1.5, "{open:?} against {roofed:?}");
}

#[test]
fn threads_do_not_change_the_picture() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(16, 10, 16));
    w.room_mut(id).fill(ivec3(0, 0, 0), ivec3(15, 0, 15), 1);
    w.room_mut(id).fill(ivec3(6, 1, 9), ivec3(8, 4, 10), 2);
    let light = Light::flood(&w, glows, |_| false);
    let camera = Camera {
        cell: Cell {
            room: id,
            at: ivec3(7, 3, 2),
        },
        pos: vec3(7.5, 3.5, 2.5),
        right: vec3(-1.0, 0.0, 0.0),
        up: vec3(0.0, 1.0, 0.0),
        fwd: vec3(0.0, 0.0, 1.0),
    };
    let frame = Frame {
        world: &w,
        overlay: None,
        light: &light,
        viewer: None,
    };
    let picture = |threads| {
        let r = Renderer::new(Settings {
            width: 64,
            height: 36,
            threads,
            ..Default::default()
        });
        let mut px = vec![0u32; 64 * 36];
        r.render(&frame, &camera, &mut px);
        px
    };
    assert_eq!(picture(1), picture(4));
}

/// The largest difference between two pixels, over their three channels.
fn apart(a: u32, b: u32) -> u32 {
    (0..3)
        .map(|i| ((a >> (i * 8)) & 0xFF).abs_diff((b >> (i * 8)) & 0xFF))
        .max()
        .unwrap()
}

/// Cut a random world in two, turn one half each of the 48 ways and glue it
/// back. A picture taken from the same place must be the same picture.
///
/// Rays agree with the uncut world bit for bit, and the tracer's tests hold
/// them to that. Shading works on the hit point, which is rounded once more
/// when it is carried across the seam, so colours may differ in the last
/// place and a shadow's edge may move by a hair. The test therefore allows
/// one grey level anywhere and a handful of pixels beyond that.
#[test]
fn cut_turn_and_glue_renders_the_same_picture_all_48_ways() {
    let n = 8;
    let (width, height) = (96u32, 54u32);
    let mut rng = Rng::new(0x1656_67B1_9E37_79F9);

    // Nothing in the lighting may depend on which way a room's axes run
    // except the sun, which is turned with its room. Materials have no
    // grain, since grain is a hash of room coordinates.
    let flat = [0.5, 0.6, 0.7];
    let env = Env {
        zenith: flat,
        horizon: flat,
        nadir: flat,
        haze: flat,
        ground_light: Env::default().sky_light,
        sun_dir: [0.41, 0.74, 0.53],
        ..Default::default()
    };
    let mut renderer = Renderer::new(Settings {
        width,
        height,
        ..Default::default()
    });
    for id in 1..=255u8 {
        let k = id as f32 / 255.0;
        renderer.set_material(
            id,
            Material {
                color: [0.3 + 0.6 * k, 0.8 - 0.5 * k, 0.4 + 0.3 * (k * 7.0).fract()],
                grain: 0.0,
                glow: if id == LAMP { 2.0 } else { 0.0 },
                ..Default::default()
            },
        );
    }

    let (mut pixels, mut within_one, mut beyond, mut lit_far_side) = (0u64, 0u64, 0u64, 0u64);
    for (ti, turn) in Orientation::all().enumerate() {
        let mut cw = CutWorld::new(n, turn);
        for z in 0..n {
            for y in 0..n {
                for x in 0..2 * n {
                    let start_row = y == n / 2 && x > 0 && x < n - 1;
                    if rng.unit() < 0.1 && !start_row {
                        cw.set(ivec3(x, y, z), rng.below(1, 200) as u8);
                    }
                }
            }
        }
        cw.set(
            ivec3(rng.below(n, 2 * n), rng.below(0, n), rng.below(0, n)),
            LAMP,
        );

        let whole_light = Light::flood(&cw.whole, glows, |_| false);
        let cut_light = Light::flood(&cw.cut, glows, |_| false);
        // The whole world's one room and the cut world's left room have the
        // same id, and the same lighting.
        assert_eq!(cw.whole_room, cw.left);
        renderer.set_env(cw.left, env);
        renderer.set_env(
            cw.right,
            Env {
                sun_dir: turn.apply_f(Vec3(env.sun_dir)).0,
                ..env
            },
        );

        // From the clear row of the left half, looking at the cut.
        let at = ivec3(1, n / 2, rng.below(2, n - 2));
        let pos = at.centre();
        let fwd = vec3(1.0, 0.1 * (rng.unit() - 0.5), 0.3 * (rng.unit() - 0.5)).normalized();
        let right = vec3(0.0, 1.0, 0.0).cross(fwd).normalized();
        let up = fwd.cross(right);
        let shot = |world, room, light| {
            let frame = Frame {
                world,
                overlay: None,
                light,
                viewer: None,
            };
            let camera = Camera {
                cell: Cell { room, at },
                pos,
                right,
                up,
                fwd,
            };
            let mut px = vec![0u32; (width * height) as usize];
            renderer.render(&frame, &camera, &mut px);
            px
        };
        let want = shot(&cw.whole, cw.whole_room, &whole_light);
        let got = shot(&cw.cut, cw.left, &cut_light);

        let mut far = 0;
        for (i, (&a, &b)) in want.iter().zip(&got).enumerate() {
            pixels += 1;
            match apart(a, b) {
                0 => {}
                1 => within_one += 1,
                d => {
                    beyond += 1;
                    far += 1;
                    assert!(
                        far <= 12,
                        "turn #{ti} {turn:?}: too many pixels differ; pixel {i} by {d}"
                    );
                }
            }
        }
        // Count frames in which the turned half is on screen and lit.
        if want.iter().any(|&p| p != want[0]) {
            lit_far_side += 1;
        }
    }
    println!("{pixels} pixels over 48 turns: {within_one} differ by one level, {beyond} by more");
    assert_eq!(lit_far_side, 48);
    // Fewer than one pixel in a thousand may differ by more than a level.
    assert!(beyond * 1000 < pixels, "{beyond} of {pixels}");
}
