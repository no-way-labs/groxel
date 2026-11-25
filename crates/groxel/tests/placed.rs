//! Placed sprites: anywhere, turned any way, at any size, and traced
//! through their own grid inside each world voxel they reach into.

use groxel::{
    Cell, EntityId, Orientation, Overlay, Placement, Ray, Scene, Skip, Sprite, TraceOpts, Vec3,
    World, ivec3, trace, vec3,
};
use std::sync::Arc;

mod common;
use common::{CutWorld, Rng};

fn scene<'a>(world: &'a World, overlay: &'a Overlay) -> Scene<'a> {
    Scene { world, overlay }
}

/// A lopsided sprite with some holes: no turn or reflection maps it onto
/// itself.
fn lumpy(rng: &mut Rng) -> Sprite {
    let dims = ivec3(3, 5, 2);
    let n = (dims[0] * dims[1] * dims[2]) as usize;
    let mut vox: Vec<u8> = (0..n).map(|_| (rng.below(0, 3) != 0) as u8 * 7).collect();
    vox[0] = 9;
    Sprite::new(dims, ivec3(1, 0, 1), vox)
}

fn axes_turned(a: f32) -> [Vec3; 3] {
    let (s, c) = a.sin_cos();
    [vec3(c, 0.0, -s), vec3(0.0, 1.0, 0.0), vec3(s, 0.0, c)]
}

#[test]
fn on_the_lattice_a_placed_sprite_is_hit_where_a_stamped_one_is() {
    let mut rng = Rng::new(11);
    let mut w = World::new();
    let id = w.add_room("r", ivec3(16, 16, 16));
    let sprite = lumpy(&mut rng);
    let at = Cell {
        room: id,
        at: ivec3(7, 5, 8),
    };
    let mut stamped = Overlay::new(&w);
    stamped.put(&w, at, Orientation::IDENTITY, &sprite, EntityId(1));
    let mut placed = Overlay::new(&w);
    let o = sprite.origin();
    placed.place(
        &w,
        &Arc::new(sprite.clone()),
        &Placement {
            at,
            pos: vec3(7.0, 5.0, 8.0),
            anchor: vec3(o[0] as f32, o[1] as f32, o[2] as f32),
            axes: [
                vec3(1.0, 0.0, 0.0),
                vec3(0.0, 1.0, 0.0),
                vec3(0.0, 0.0, 1.0),
            ],
            scale: 1.0,
        },
        EntityId(1),
    );
    let mut hits = 0;
    for _ in 0..2000 {
        // From a point well clear of the sprite.
        let origin = loop {
            let p = vec3(rng.unit() * 16.0, rng.unit() * 16.0, rng.unit() * 16.0);
            if (p - vec3(7.5, 7.5, 8.5)).length() > 5.0 {
                break p;
            }
        };
        // Aimed at a point of the sprite's box, give or take a voxel.
        let target = vec3(
            5.0 + rng.unit() * 5.0,
            4.0 + rng.unit() * 7.0,
            6.0 + rng.unit() * 4.0,
        );
        let ray = Ray::new(id, origin, (target - origin).normalized());
        let a = trace(&scene(&w, &stamped), ray, &TraceOpts::default());
        let b = trace(&scene(&w, &placed), ray, &TraceOpts::default());
        assert_eq!(a.entity.is_some(), b.part.is_some(), "{ray:?}");
        if let Some(part) = b.part {
            hits += 1;
            assert!((a.t - b.t).abs() < 1e-4, "{} vs {}", a.t, b.t);
            assert_eq!(part.voxel, a.cell.at - at.at + o);
            assert_eq!(a.material, b.material);
            assert_eq!(part.face, a.face);
        }
    }
    assert!(hits > 100, "{hits}");
}

#[test]
fn a_placed_sprite_across_a_turned_seam_matches_the_uncut_world_all_48_ways() {
    let n = 8;
    let mut rng = Rng::new(5);
    let sprite = Arc::new(lumpy(&mut rng));
    // Straddling x = n, turned off the lattice, larger than its voxels.
    let pos = vec3(n as f32 - 0.6, 2.3, 4.1);
    let anchor = vec3(1.4, 0.2, 0.9);
    let (axes, scale) = (axes_turned(0.61), 0.8);
    let mut through = 0;
    for (k, turn) in Orientation::all().enumerate() {
        let cw = CutWorld::new(n, turn);
        let at = pos.floor();
        let whole_at = Cell {
            room: cw.whole_room,
            at,
        };
        let cut_at = Cell { room: cw.left, at };
        let mut whole = Overlay::new(&cw.whole);
        let mut cut = Overlay::new(&cw.cut);
        let p = |at| Placement {
            at,
            pos,
            anchor,
            axes,
            scale,
        };
        whole.place(&cw.whole, &sprite, &p(whole_at), EntityId(1));
        cut.place(&cw.cut, &sprite, &p(cut_at), EntityId(1));
        for i in 0..400 {
            let origin = vec3(
                rng.unit() * (n as f32 - 3.0),
                rng.unit() * n as f32,
                rng.unit() * n as f32,
            );
            // Most rays aimed at a point of the sprite's neighbourhood, on
            // either side of the seam; the rest anywhere.
            let dir = if i % 4 == 0 {
                rng.direction()
            } else {
                let target = vec3(
                    n as f32 - 2.5 + rng.unit() * 5.0,
                    1.5 + rng.unit() * 5.5,
                    2.5 + rng.unit() * 3.5,
                );
                (target - origin).normalized()
            };
            let a = trace(
                &scene(&cw.whole, &whole),
                Ray::new(cw.whole_room, origin, dir),
                &TraceOpts::default(),
            );
            let b = trace(
                &scene(&cw.cut, &cut),
                Ray::new(cw.left, origin, dir),
                &TraceOpts::default(),
            );
            let ctx = format!("turn #{k}, from {origin:?} along {dir:?}");
            assert_eq!(a.part.is_some(), b.part.is_some(), "{ctx}");
            let (Some(pa), Some(pb)) = (a.part, b.part) else {
                continue;
            };
            assert!((a.t - b.t).abs() < 1e-3, "{ctx}: {} vs {}", a.t, b.t);
            assert_eq!(pa.voxel, pb.voxel, "{ctx}");
            assert_eq!(pa.face, pb.face, "{ctx}");
            // The normal is the same direction, read in each room's frame.
            let na = if b.cell.room == cw.right {
                turn.apply_f(pa.normal)
            } else {
                pa.normal
            };
            assert!((na - pb.normal).length() < 1e-4, "{ctx}");
            through += (b.cell.room == cw.right) as u32;
        }
    }
    // The test means nothing unless rays met the sprite beyond the seam.
    assert!(through > 300, "{through}");
}

#[test]
fn a_viewer_looks_past_its_own_placed_figure_up_close() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(16, 16, 16));
    let sprite = Arc::new(Sprite::new(ivec3(1, 1, 1), ivec3(0, 0, 0), vec![5]));
    let mut overlay = Overlay::new(&w);
    let at = Cell {
        room: id,
        at: ivec3(8, 8, 8),
    };
    overlay.place(
        &w,
        &sprite,
        &Placement {
            at,
            pos: vec3(8.2, 8.2, 8.2),
            anchor: vec3(0.0, 0.0, 0.0),
            axes: axes_turned(0.3),
            scale: 2.0,
        },
        EntityId(4),
    );
    let ray = Ray::new(id, vec3(8.45, 8.45, 2.0), vec3(0.0, 0.0, 1.0));
    let scene = scene(&w, &overlay);
    let seen = trace(&scene, ray, &TraceOpts::default());
    assert_eq!(seen.entity, Some(EntityId(4)));
    assert!(seen.t > 6.0 && seen.t < 7.0, "{}", seen.t);
    let near = TraceOpts {
        skip: Some(Skip {
            entity: EntityId(4),
            near: 10.0,
        }),
        ..Default::default()
    };
    assert!(trace(&scene, ray, &near).part.is_none());
    let far = TraceOpts {
        skip: Some(Skip {
            entity: EntityId(4),
            near: 3.0,
        }),
        ..Default::default()
    };
    assert!(trace(&scene, ray, &far).part.is_some());
}
