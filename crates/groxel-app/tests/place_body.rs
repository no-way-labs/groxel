//! `place_body` draws the figure `put_body` draws, wherever both can: at a
//! voxel's middle, facing a quarter turn, in any of the 48 frames.

use groxel::{Orientation, Overlay, Ray, Scene, TraceOpts, World, ivec3, trace, vec3};
use groxel_app::{VIEWER, place_body, put_body, you};
use groxel_body::Body;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

#[test]
fn on_the_lattice_place_body_draws_what_put_body_draws() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(30, 30, 30));
    let sprite = &you()[0];
    let shared = Arc::new(sprite.clone());
    let mut seed = 7u32;
    let mut unit = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed & 0xFFFF) as f32 / 65535.0
    };
    let mut hits = 0;
    for (k, frame) in Orientation::all().enumerate() {
        for quarter in 0..4 {
            let mut body = Body::new(id, vec3(15.5, 15.5, 15.5));
            body.frame = frame;
            body.yaw = quarter as f32 * FRAC_PI_2;
            // Feet on a voxel's face, in the middle of it across.
            let up = body.up_face();
            body.pos[up.axis()] = 15.0;
            let mut a = Overlay::new(&w);
            put_body(&mut a, &w, &body, sprite, VIEWER);
            let mut b = Overlay::new(&w);
            place_body(&mut b, &w, &body, &shared, 1.0, VIEWER);
            for _ in 0..40 {
                let origin = vec3(unit() * 30.0, unit() * 30.0, unit() * 30.0);
                if (origin - vec3(15.5, 15.5, 15.5)).length() < 9.0 {
                    continue;
                }
                let target = vec3(
                    12.0 + unit() * 7.0,
                    12.0 + unit() * 7.0,
                    12.0 + unit() * 7.0,
                );
                let ray = Ray::new(id, origin, (target - origin).normalized());
                let opts = TraceOpts::default();
                let ha = trace(
                    &Scene {
                        world: &w,
                        overlay: &a,
                    },
                    ray,
                    &opts,
                );
                let hb = trace(
                    &Scene {
                        world: &w,
                        overlay: &b,
                    },
                    ray,
                    &opts,
                );
                let ctx = format!("frame #{k}, quarter {quarter}, {ray:?}");
                assert_eq!(ha.entity.is_some(), hb.part.is_some(), "{ctx}");
                if hb.part.is_some() {
                    hits += 1;
                    assert!((ha.t - hb.t).abs() < 1e-3, "{ctx}: {} vs {}", ha.t, hb.t);
                    assert_eq!(ha.material, hb.material, "{ctx}");
                }
            }
        }
    }
    assert!(hits > 1000, "{hits}");
}
