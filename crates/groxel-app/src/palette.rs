//! How the materials look, and which of them are lamps.

use crate::mat;
use groxel_render::{Material, Pattern};
use std::sync::LazyLock;

static PALETTE: LazyLock<[Material; 256]> = LazyLock::new(build);

/// How each material of the demo worlds looks, indexed by material id.
///
/// The colours are chosen as they should look on screen, and are stored here
/// in linear light, which is what the renderer works in.
pub fn palette() -> &'static [Material; 256] {
    &PALETTE
}

/// True if a material is a lamp. This is the predicate the light flood
/// wants: a material glows if the palette gives it any glow.
pub fn glows(material: u8) -> bool {
    PALETTE[material as usize].glow > 0.0
}

/// A colour as it should look, converted to linear light. The renderer's
/// display curve is a square root, so this is a square.
pub fn lin(c: [f32; 3]) -> [f32; 3] {
    [c[0] * c[0], c[1] * c[1], c[2] * c[2]]
}

/// A vector scaled to length one.
pub fn norm(v: [f32; 3]) -> [f32; 3] {
    let k = 1.0 / (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] * k, v[1] * k, v[2] * k]
}

fn build() -> [Material; 256] {
    let plain = |color: [f32; 3], grain: f32| Material {
        color,
        grain,
        ..Default::default()
    };
    let patterned = |color: [f32; 3], pattern: Pattern, scale: i32, grain: f32| Material {
        color,
        pattern,
        scale,
        grain,
        ..Default::default()
    };
    let glowing = |color: [f32; 3], glow: f32| Material {
        color,
        glow,
        grain: 0.0,
        ..Default::default()
    };

    let mut table = [Material::default(); 256];
    let mut set = |id: u8, material: Material| table[id as usize] = material;
    set(
        mat::LIMESTONE,
        patterned([0.78, 0.74, 0.66], Pattern::Tile, 4, 0.06),
    );
    set(mat::SLATE, plain([0.30, 0.32, 0.36], 0.08));
    set(
        mat::BRICK,
        patterned([0.62, 0.42, 0.33], Pattern::Brick, 4, 0.10),
    );
    set(mat::MARBLE, plain([0.86, 0.85, 0.82], 0.03));
    set(
        mat::WOOD,
        patterned([0.45, 0.30, 0.18], Pattern::Plank, 8, 0.12),
    );
    set(mat::LACQUER, plain([0.74, 0.13, 0.10], 0.04));
    set(mat::LAMP, glowing([1.0, 0.82, 0.48], 1.9));
    set(mat::ROBE, plain([0.50, 0.36, 0.22], 0.05));
    set(mat::SKIN, plain([0.85, 0.66, 0.52], 0.0));
    set(mat::COWL, plain([0.33, 0.23, 0.14], 0.05));
    set(mat::YOU, plain([0.18, 0.36, 0.62], 0.04));
    set(mat::YOU_DARK, plain([0.10, 0.20, 0.38], 0.04));
    set(mat::IRON, plain([0.16, 0.16, 0.18], 0.05));
    set(mat::PLASTER, plain([0.84, 0.80, 0.72], 0.03));
    set(
        mat::CHECKER,
        Material {
            color2: [0.20, 0.20, 0.23],
            ..patterned([0.85, 0.83, 0.78], Pattern::Checker, 4, 0.03)
        },
    );
    set(mat::GOLD, plain([0.86, 0.66, 0.22], 0.05));
    set(mat::MARKER, glowing([0.30, 0.95, 0.75], 1.5));
    set(mat::WINDOW, glowing([1.0, 0.74, 0.36], 1.25));
    set(mat::AZURE, plain([0.16, 0.42, 0.78], 0.04));
    set(
        mat::CARPET,
        patterned([0.62, 0.12, 0.14], Pattern::Tile, 2, 0.08),
    );

    // Colours above are written as they should look; the renderer works in
    // linear light. This covers every entry, the unused ones included.
    for one in &mut table {
        one.color = lin(one.color);
        one.color2 = lin(one.color2);
    }
    table
}
