//! The figures: monks who walk the worlds, and you.
//!
//! Each is seven voxels tall and faces +z. Layers run bottom to top, rows
//! back to front.

use crate::mat;
use groxel::{EntityId, Orientation, Overlay, Placement, Sprite, World, vec3};
use groxel_body::Body;
use std::f32::consts::FRAC_PI_2;
use std::sync::{Arc, LazyLock};

const FIGURE_KEY: [(char, u8); 3] = [('R', mat::ROBE), ('H', mat::COWL), ('F', mat::SKIN)];
const YOU_KEY: [(char, u8); 3] = [('R', mat::YOU), ('H', mat::YOU_DARK), ('F', mat::SKIN)];

const FIGURE_LAYERS: [[&str; 3]; 7] = [
    ["RRR", "RRR", "RRR"],
    ["RRR", "RRR", "RRR"],
    [".R.", "RRR", "RRR"],
    [".R.", "RRR", ".R."],
    ["RRR", "RRR", ".R."],
    ["HHH", "HHH", "HFH"],
    [".H.", "HHH", ".H."],
];
const STRIDE_LAYERS: [[&str; 3]; 7] = [
    ["RR.", "RRR", ".RR"],
    ["RRR", "RRR", "RRR"],
    [".R.", "RRR", "RRR"],
    [".R.", "RRR", ".R."],
    ["RRR", "RRR", ".R."],
    ["HHH", "HHH", "HFH"],
    [".H.", "HHH", ".H."],
];

fn sprite(layers: &[[&str; 3]; 7], key: &[(char, u8)]) -> Sprite {
    let rows: Vec<&[&str]> = layers.iter().map(|layer| &layer[..]).collect();
    Sprite::from_layers(&rows, key)
}

static MONK: LazyLock<[Sprite; 2]> = LazyLock::new(|| {
    [
        sprite(&FIGURE_LAYERS, &FIGURE_KEY),
        sprite(&STRIDE_LAYERS, &FIGURE_KEY),
    ]
});
static YOU: LazyLock<[Sprite; 2]> = LazyLock::new(|| {
    [
        sprite(&FIGURE_LAYERS, &YOU_KEY),
        sprite(&STRIDE_LAYERS, &YOU_KEY),
    ]
});

/// A monk: standing, and in mid stride. Alternating the two as a figure
/// moves makes it walk.
pub fn monk() -> &'static [Sprite; 2] {
    &MONK
}

/// The viewer's own figure, in the same two poses as [`monk`].
pub fn you() -> &'static [Sprite; 2] {
    &YOU
}

/// Put a sprite into an overlay where a body stands, facing the nearest
/// quarter turn to where the body faces.
///
/// The sprite's origin goes in the voxel that holds the body's feet. That
/// voxel is found by walking the lattice from the body's middle, and the
/// sprite is turned by whatever that walk crosses, by the body's own frame
/// and by its heading. So a figure standing on a wall, or half way through a
/// door, is drawn the right way round.
///
/// Nothing is drawn if the walk to the feet leaves the world.
pub fn put_body(
    overlay: &mut Overlay,
    world: &World,
    body: &Body,
    sprite: &Sprite,
    entity: EntityId,
) {
    let up = body.up_face();
    let mut feet = body.pos;
    feet[up.axis()] += up.sign() as f32 * 0.5;
    let from = body.anchor();
    let Some((there, turn)) = world.walk(from, feet.floor() - from.at) else {
        return;
    };
    let quarter = (body.yaw / FRAC_PI_2).round() as i32;
    let orient = Orientation::quarter_turn(1, quarter)
        .then(body.frame)
        .then(turn);
    overlay.put(world, there, orient, sprite, entity);
}

/// Place a sprite where a body stands, exactly: at the body's position, not
/// the voxel it is in, and facing its heading, not the nearest quarter
/// turn. So a figure that walks or turns moves smoothly. `scale` is sprite
/// voxels to one world voxel: 1 draws [`put_body`]'s figure at its size.
///
/// The middle of the sprite's foot (its origin voxel's bottom face) goes at
/// the body's feet, and the sprite is turned by the body's frame and its
/// heading, as [`put_body`] turns it. It is placed from the body's middle,
/// so a figure half way through a door is drawn on both sides.
pub fn place_body(
    overlay: &mut Overlay,
    world: &World,
    body: &Body,
    sprite: &Arc<Sprite>,
    scale: f32,
    entity: EntityId,
) {
    let up = body.up_face().normal();
    let up = vec3(up[0] as f32, up[1] as f32, up[2] as f32);
    let half = body.height * 0.5;
    let o = sprite.origin();
    // The heading turns the sprite about its own +y, as a quarter turn of
    // `put_body` does: +x to (cos, 0, -sin), +z to (sin, 0, cos).
    let (s, c) = body.yaw.sin_cos();
    let local = [vec3(c, 0.0, -s), vec3(0.0, 1.0, 0.0), vec3(s, 0.0, c)];
    overlay.place(
        world,
        sprite,
        &Placement {
            at: body.anchor(),
            pos: body.pos + up * half,
            anchor: vec3(
                o[0] as f32 + 0.5,
                o[1] as f32 + half * scale,
                o[2] as f32 + 0.5,
            ),
            axes: local.map(|a| body.frame.apply_f(a)),
            scale,
        },
        entity,
    );
}
