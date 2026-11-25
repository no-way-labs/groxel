//! Light. The central test is the one the tracer has: a world cut in two,
//! with one half turned any of the 48 ways and glued back, is lit exactly as
//! the uncut world is.

use groxel::{Cell, Face, FaceRect, Orientation, World, ivec3};
use groxel_light::{Light, REACH};

#[path = "../../groxel/tests/common/mod.rs"]
mod common;
use common::{CutWorld, Rng};

const LAMP: u8 = 250;

fn glows(material: u8) -> bool {
    material == LAMP
}

#[test]
fn a_lamp_lights_its_room_and_falls_off_with_distance() {
    let mut w = World::new();
    let id = w.add_room("r", ivec3(24, 8, 8));
    let r = w.room_mut(id);
    // A closed box.
    r.fill(ivec3(0, 0, 0), ivec3(23, 0, 7), 1);
    r.fill(ivec3(0, 7, 0), ivec3(23, 7, 7), 1);
    r.fill(ivec3(0, 0, 0), ivec3(0, 7, 7), 1);
    r.fill(ivec3(23, 0, 0), ivec3(23, 7, 7), 1);
    r.fill(ivec3(0, 0, 0), ivec3(23, 7, 0), 1);
    r.fill(ivec3(0, 0, 7), ivec3(23, 7, 7), 1);
    r.set(ivec3(3, 3, 3), LAMP);

    let light = Light::flood(&w, glows, |_| false);
    let at = |x, y, z| Cell {
        room: id,
        at: ivec3(x, y, z),
    };
    assert_eq!(light.lamp(at(4, 3, 3)), REACH);
    assert_eq!(light.lamp(at(13, 3, 3)), REACH - 9);
    // No sky gets into a closed box, and no light into its walls.
    assert_eq!(light.sky(at(13, 3, 3)), 0);
    assert_eq!(light.lamp(at(0, 3, 3)), 0);
}

#[test]
fn light_goes_through_a_seam() {
    let mut w = World::new();
    let a = w.add_room("a", ivec3(6, 3, 3));
    let b = w.add_room("b", ivec3(6, 3, 3));
    w.room_mut(a).set(ivec3(0, 1, 1), LAMP);
    w.glue(
        FaceRect {
            room: a,
            min: ivec3(5, 0, 0),
            max: ivec3(5, 2, 2),
            face: Face::PosX,
        },
        FaceRect {
            room: b,
            min: ivec3(0, 0, 0),
            max: ivec3(0, 2, 2),
            face: Face::NegX,
        },
        Orientation::IDENTITY,
    )
    .unwrap();

    let light = Light::flood(&w, glows, |_| false);
    // The lamp is at x = 0 in a; x = 1 has full light; b's x = 2 is seven
    // steps further on.
    let in_b = |x| Cell {
        room: b,
        at: ivec3(x, 1, 1),
    };
    assert_eq!(light.lamp(in_b(2)), REACH - 7);
    // b's far end is open to the sky.
    assert_eq!(light.sky(in_b(5)), REACH);
}

#[test]
fn an_outdoors_room_is_all_sky() {
    let mut w = World::new();
    let id = w.add_room("yard", ivec3(4, 4, 4));
    w.room_mut(id).fill(ivec3(0, 0, 0), ivec3(3, 3, 0), 1);
    let light = Light::flood(&w, glows, |room| room == id);
    assert_eq!(
        light.sky(Cell {
            room: id,
            at: ivec3(2, 2, 2)
        }),
        REACH
    );
}

/// A closed room whose only opening is a doorway glued to an outdoors room
/// is lit by the sky through the doorway, as it would be through an
/// opening onto its own open edge.
#[test]
fn sky_comes_through_a_seam_from_outdoors() {
    let mut w = World::new();
    let yard = w.add_room("yard", ivec3(6, 3, 3));
    let hut = w.add_room("hut", ivec3(10, 5, 5));
    // Walls all round; the inside is x 1..=8, y 1..=3, z 1..=3.
    let r = w.room_mut(hut);
    r.fill(ivec3(0, 0, 0), ivec3(9, 4, 4), 1);
    r.fill(ivec3(1, 1, 1), ivec3(8, 3, 3), 0);
    w.glue(
        FaceRect {
            room: yard,
            min: ivec3(5, 0, 0),
            max: ivec3(5, 2, 2),
            face: Face::PosX,
        },
        FaceRect {
            room: hut,
            min: ivec3(1, 1, 1),
            max: ivec3(1, 3, 3),
            face: Face::NegX,
        },
        Orientation::IDENTITY,
    )
    .unwrap();

    let light = Light::flood(&w, glows, |room| room == yard);
    let at = |x| Cell {
        room: hut,
        at: ivec3(x, 2, 2),
    };
    assert_eq!(light.sky(at(1)), REACH - 1);
    assert_eq!(light.sky(at(8)), REACH - 8);

    // Shut the doorway and the hut is dark.
    w.room_mut(yard).fill(ivec3(5, 0, 0), ivec3(5, 2, 2), 1);
    let light = Light::flood(&w, glows, |room| room == yard);
    assert_eq!(light.sky(at(1)), 0);
}

/// Cut a random world in two, turn one half each of the 48 ways and glue it
/// back. Every voxel must have the lamp and sky light it has in the uncut
/// world.
#[test]
fn cut_turn_and_glue_is_lit_the_same_all_48_ways() {
    let n = 6;
    let mut rng = Rng::new(0xA54F_F53A_5F1D_36F1);
    let mut lit_through_the_seam = 0;
    let mut levels_seen = [false; REACH as usize + 1];

    for (ti, turn) in Orientation::all().enumerate() {
        let mut cw = CutWorld::new(n, turn);
        // Enough solid to make light go round corners.
        for z in 0..n {
            for y in 0..n {
                for x in 0..2 * n {
                    if rng.unit() < 0.35 {
                        cw.set(ivec3(x, y, z), rng.below(1, 200) as u8);
                    }
                }
            }
        }
        // Lamps in one half only, so the other half is lit through the seam.
        // Which half alternates.
        let lamps_on_left = ti % 2 == 0;
        for _ in 0..2 {
            let x = rng.below(0, n) + if lamps_on_left { 0 } else { n };
            cw.set(ivec3(x, rng.below(0, n), rng.below(0, n)), LAMP);
        }

        let want = Light::flood(&cw.whole, glows, |_| false);
        let got = Light::flood(&cw.cut, glows, |_| false);

        for z in 0..n {
            for y in 0..n {
                for x in 0..2 * n {
                    let at = ivec3(x, y, z);
                    let whole = Cell {
                        room: cw.whole_room,
                        at,
                    };
                    let (room, there) = cw.cell_in_cut(at);
                    let cut = Cell { room, at: there };
                    let ctx = format!("turn #{ti} {turn:?}, voxel {at:?}");
                    assert_eq!(want.lamp(whole), got.lamp(cut), "lamp, {ctx}");
                    assert_eq!(want.sky(whole), got.sky(cut), "sky, {ctx}");

                    levels_seen[want.lamp(whole) as usize] = true;
                    let far_side = (x < n) != lamps_on_left;
                    if far_side && want.lamp(whole) > 0 {
                        lit_through_the_seam += 1;
                    }
                }
            }
        }
    }
    // The test means nothing unless light did cross the seam, and at more
    // than a couple of levels.
    assert!(lit_through_the_seam > 2000, "{lit_through_the_seam}");
    let distinct = levels_seen.iter().filter(|&&seen| seen).count();
    assert!(distinct > 12, "{distinct}");
}
