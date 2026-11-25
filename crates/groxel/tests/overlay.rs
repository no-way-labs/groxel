//! The overlay: sprites placed by walking the lattice, and rays that see them.

use groxel::{
    Cell, EntityId, Face, FaceRect, HitKind, Orientation, Overlay, Ray, Scene, Skip, Sprite, Stamp,
    TraceOpts, World, ivec3, trace, vec3,
};

mod common;
use common::{CutWorld, Rng};

#[test]
fn a_sprite_straddles_a_wrap_seam() {
    let mut w = World::new();
    let id = w.add_room("ring", ivec3(8, 6, 6));
    w.wrap(id, 0).unwrap();
    // Three voxels along z. Turned a quarter about y, that is along x.
    let bar = Sprite::new(ivec3(1, 1, 3), ivec3(0, 0, 1), vec![7, 7, 7]);
    let mut over = Overlay::new(&w);
    let at = |x| Cell {
        room: id,
        at: ivec3(x, 2, 2),
    };
    over.put(
        &w,
        at(7),
        Orientation::quarter_turn(1, 1),
        &bar,
        EntityId(1),
    );
    assert_eq!(over.len(), 3);
    for x in [6, 7, 0] {
        assert_eq!(over.get(at(x)).map(|s| s.material), Some(7), "x = {x}");
    }
    assert_eq!(over.get(at(1)), None);
    // The world itself is untouched.
    assert!(!w.room(id).is_solid(ivec3(7, 2, 2)));

    over.clear();
    assert!(over.is_empty());
    assert_eq!(over.get(at(7)), None);
}

#[test]
fn a_sprite_does_not_replace_the_world_or_an_earlier_sprite() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(6, 3, 3));
    w.room_mut(id).set(ivec3(2, 1, 1), 5);
    let row = Sprite::new(ivec3(3, 1, 1), ivec3(0, 0, 0), vec![7, 8, 9]);
    let mut over = Overlay::new(&w);
    let at = |x| Cell {
        room: id,
        at: ivec3(x, 1, 1),
    };
    over.put(&w, at(1), Orientation::IDENTITY, &row, EntityId(1));
    over.put(&w, at(3), Orientation::IDENTITY, &row, EntityId(2));
    assert_eq!(over.get(at(1)).unwrap().material, 7);
    assert_eq!(over.get(at(2)), None); // the world is solid here
    assert_eq!(over.get(at(3)).unwrap().entity, EntityId(1)); // first come
    assert_eq!(over.get(at(4)).unwrap().entity, EntityId(2));
    // Past the edge of the room nothing is stamped.
    over.put(&w, at(5), Orientation::IDENTITY, &row, EntityId(3));
    assert_eq!(over.len(), 4);
}

/// `iter` lists exactly what `get` finds at every voxel, in rooms of
/// different sizes, with sprites that straddle the seam between them.
#[test]
fn iter_lists_every_stamp_and_nothing_else() {
    let mut w = World::new();
    let a = w.add_room("a", ivec3(7, 5, 9));
    let b = w.add_room("b", ivec3(4, 11, 6));
    w.glue_up(
        FaceRect {
            room: a,
            min: ivec3(6, 0, 0),
            max: ivec3(6, 4, 3),
            face: Face::PosX,
        },
        FaceRect {
            room: b,
            min: ivec3(0, 0, 0),
            max: ivec3(3, 4, 0),
            face: Face::NegZ,
        },
        Face::PosY,
        Face::PosY,
    )
    .unwrap();
    let bar = Sprite::new(ivec3(5, 2, 1), ivec3(2, 0, 0), (1..=10).collect());
    let mut over = Overlay::new(&w);
    assert_eq!(over.iter().count(), 0);
    let mut rng = Rng::new(7);
    for e in 0..12 {
        let room = if e % 2 == 0 { a } else { b };
        let d = w.room(room).dims();
        let at = ivec3(rng.below(0, d[0]), rng.below(0, d[1]), rng.below(0, d[2]));
        let turn = Orientation::all().nth(rng.below(0, 48) as usize).unwrap();
        over.put(&w, Cell { room, at }, turn, &bar, EntityId(e));
    }

    let key = |(c, _): &(Cell, Stamp)| (c.room.0, c.at.0);
    let mut listed: Vec<_> = over.iter().collect();
    listed.sort_by_key(key);
    let mut found = Vec::new();
    for room in [a, b] {
        let d = w.room(room).dims();
        for z in 0..d[2] {
            for y in 0..d[1] {
                for x in 0..d[0] {
                    let cell = Cell {
                        room,
                        at: ivec3(x, y, z),
                    };
                    if let Some(stamp) = over.get(cell) {
                        found.push((cell, stamp));
                    }
                }
            }
        }
    }
    found.sort_by_key(key);
    assert_eq!(listed, found);
    assert_eq!(listed.len(), over.len());
    assert!(listed.iter().any(|(c, _)| c.room == a));
    assert!(listed.iter().any(|(c, _)| c.room == b));

    over.clear();
    assert_eq!(over.iter().count(), 0);
}

#[test]
fn a_ray_sees_a_sprite_and_can_look_past_its_own() {
    let mut w = World::new();
    let id = w.add_room("ring", ivec3(12, 3, 3));
    w.wrap(id, 0).unwrap();
    let dot = Sprite::new(ivec3(1, 1, 1), ivec3(0, 0, 0), vec![7]);
    let mut over = Overlay::new(&w);
    let me = EntityId(4);
    over.put(
        &w,
        Cell {
            room: id,
            at: ivec3(5, 1, 1),
        },
        Orientation::IDENTITY,
        &dot,
        me,
    );
    let scene = Scene {
        world: &w,
        overlay: &over,
    };
    let ray = Ray::new(id, vec3(2.5, 1.5, 1.5), vec3(1.0, 0.0, 0.0));

    // The world alone is empty.
    assert_eq!(trace(&w, ray, &TraceOpts::default()).kind, HitKind::Far);

    // The scene has the dot in it, three voxels on.
    let hit = trace(&scene, ray, &TraceOpts::default());
    assert_eq!(
        (hit.kind, hit.material, hit.entity),
        (HitKind::Solid, 7, Some(me))
    );
    assert!((hit.t - 2.5).abs() < 1e-5);
    assert_eq!(hit.crossings, 0);

    // Looking past my own voxels, I see them again once round the ring.
    let opts = TraceOpts {
        skip: Some(Skip {
            entity: me,
            near: 6.0,
        }),
        ..Default::default()
    };
    let hit = trace(&scene, ray, &opts);
    assert_eq!((hit.kind, hit.entity), (HitKind::Solid, Some(me)));
    assert!((hit.t - 14.5).abs() < 1e-4);
    assert_eq!(hit.crossings, 1);

    // Another entity's skip does not hide them.
    let opts = TraceOpts {
        skip: Some(Skip {
            entity: EntityId(9),
            near: 6.0,
        }),
        ..Default::default()
    };
    assert!((trace(&scene, ray, &opts).t - 2.5).abs() < 1e-5);
}

/// Cut a world in two, turn one half each of the 48 ways and glue it back.
/// A sprite placed across the cut must occupy the same voxels as in the uncut
/// world, and rays must see it the same.
#[test]
fn a_sprite_across_a_turned_seam_matches_the_uncut_world_all_48_ways() {
    let n = 6;
    let mut rng = Rng::new(0xC2B2_AE3D_27D4_EB4F);
    // No two voxels alike, so a sprite put down the wrong way round shows.
    let dims = ivec3(3, 4, 2);
    let sprite = Sprite::new(dims, ivec3(1, 0, 1), (0..24).map(|i| 100 + i).collect());
    let orients: Vec<Orientation> = Orientation::all().collect();
    let mut straddling = 0;
    let mut sprite_hits = 0;
    let (mut looked_past, mut own_beyond_the_cut) = (0, 0);

    for (ti, turn) in Orientation::all().enumerate() {
        let mut cw = CutWorld::new(n, turn);
        for z in 0..n {
            for y in 0..n {
                for x in 0..2 * n {
                    let start_row = y == n / 2 && x > 0 && x < n - 1;
                    if rng.unit() < 0.06 && !start_row {
                        cw.set(ivec3(x, y, z), rng.below(1, 90) as u8);
                    }
                }
            }
        }

        let mut whole = Overlay::new(&cw.whole);
        let mut cut = Overlay::new(&cw.cut);
        for e in 0..4 {
            // Anchors near the cut, on either side of it.
            let anchor = ivec3(
                rng.below(n - 2, n + 2),
                rng.below(1, n - 1),
                rng.below(1, n - 1),
            );
            let orient = orients[rng.below(0, 48) as usize];
            whole.put(
                &cw.whole,
                Cell {
                    room: cw.whole_room,
                    at: anchor,
                },
                orient,
                &sprite,
                EntityId(e),
            );
            let (room, at) = cw.cell_in_cut(anchor);
            // In the turned half the sprite's axes are turned with it.
            let there = if anchor[0] < n {
                orient
            } else {
                orient.then(turn)
            };
            cut.put(&cw.cut, Cell { room, at }, there, &sprite, EntityId(e));
        }

        let (mut on_left, mut on_right) = (0, 0);
        for z in 0..n {
            for y in 0..n {
                for x in 0..2 * n {
                    let at = ivec3(x, y, z);
                    let want = whole.get(Cell {
                        room: cw.whole_room,
                        at,
                    });
                    let (room, there) = cw.cell_in_cut(at);
                    let got = cut.get(Cell { room, at: there });
                    assert_eq!(want, got, "turn #{ti} {turn:?}, voxel {at:?}");
                    if want.is_some() {
                        if x < n { on_left += 1 } else { on_right += 1 }
                    }
                }
            }
        }
        assert_eq!(whole.len(), cut.len());
        if on_left > 0 && on_right > 0 {
            straddling += 1;
        }

        let whole_scene = Scene {
            world: &cw.whole,
            overlay: &whole,
        };
        let cut_scene = Scene {
            world: &cw.cut,
            overlay: &cut,
        };
        for k in 0..200 {
            let cell = ivec3(rng.below(1, n - 1), n / 2, rng.below(0, n));
            let origin = vec3(
                cell[0] as f32 + 0.1 + 0.8 * rng.unit(),
                cell[1] as f32 + 0.1 + 0.8 * rng.unit(),
                cell[2] as f32 + 0.1 + 0.8 * rng.unit(),
            );
            let dir = rng.direction();
            let opts = TraceOpts::default();
            let want = trace(&whole_scene, Ray::new(cw.whole_room, origin, dir), &opts);
            let got = trace(&cut_scene, Ray::new(cw.left, origin, dir), &opts);
            let ctx = format!("turn #{ti} {turn:?}, ray {k}");
            assert_eq!(want.kind, got.kind, "{ctx}");
            assert_eq!(want.t.to_bits(), got.t.to_bits(), "{ctx}");
            assert_eq!(want.material, got.material, "{ctx}");
            assert_eq!(want.entity, got.entity, "{ctx}");
            if want.entity.is_some() {
                sprite_hits += 1;
            }

            // The same ray, looking past one of the sprites as its viewer
            // would. Whether a voxel is looked past must not depend on which
            // side of the cut it is on.
            // Mostly the sprite the ray stopped on, so that the rule decides.
            let any = EntityId(rng.below(0, 4) as u32);
            let me = want.entity.filter(|_| k % 4 != 0).unwrap_or(any);
            let near = [0.0, 2.0, 5.0][k % 3];
            let opts = TraceOpts {
                skip: Some(Skip { entity: me, near }),
                ..Default::default()
            };
            let past = trace(&whole_scene, Ray::new(cw.whole_room, origin, dir), &opts);
            let got = trace(&cut_scene, Ray::new(cw.left, origin, dir), &opts);
            let ctx = format!("turn #{ti} {turn:?}, ray {k}, looking past {me:?} to {near}");
            assert_eq!(past.kind, got.kind, "{ctx}");
            assert_eq!(past.t.to_bits(), got.t.to_bits(), "{ctx}");
            assert_eq!(past.steps, got.steps, "{ctx}");
            assert_eq!(past.material, got.material, "{ctx}");
            assert_eq!(past.entity, got.entity, "{ctx}");
            if past.t != want.t {
                looked_past += 1;
            }
            if got.entity == Some(me) && got.crossings > 0 {
                own_beyond_the_cut += 1;
            }
        }
    }
    // The test means nothing unless sprites did lie across the cut and rays
    // did hit them, some rays did look past a sprite, and some stopped on
    // the viewer's own sprite beyond the cut.
    assert!(straddling > 40, "{straddling}");
    assert!(sprite_hits > 1000, "{sprite_hits}");
    assert!(looked_past > 100, "{looked_past}");
    assert!(own_beyond_the_cut > 50, "{own_beyond_the_cut}");
}
