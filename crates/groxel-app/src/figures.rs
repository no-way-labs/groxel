//! The figures: monks who walk the worlds, and you.
//!
//! Each is seven voxels tall and faces +z. Layers run bottom to top, rows
//! back to front.

use crate::mat;
use groxel::{EntityId, Orientation, Overlay, Sprite, World};
use groxel_body::Body;
use std::f32::consts::FRAC_PI_2;
use std::sync::LazyLock;

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
