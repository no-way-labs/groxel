//! What the engine's data costs in memory.
//!
//! ```text
//! cargo run --release -p groxel-experiments --bin sizes
//! ```
//!
//! Sizes of the types come from the compiler. Per-voxel figures are read off
//! the storage each crate declares: the core keeps one byte of material and
//! one byte of link bits per voxel, the light crate one byte of lamp light
//! and one of sky light, and an overlay one bit.

use groxel::{Cell, Cursor, Hit, LatticeMap, Orientation, Pose, Ray, Seam, Stamp};
use std::mem::size_of;

fn main() {
    println!("type            bytes");
    println!("Orientation     {:>5}", size_of::<Orientation>());
    println!("LatticeMap      {:>5}", size_of::<LatticeMap>());
    println!("Seam            {:>5}", size_of::<Seam>());
    println!("Cell            {:>5}", size_of::<Cell>());
    println!("Pose            {:>5}", size_of::<Pose>());
    println!("Ray             {:>5}", size_of::<Ray>());
    println!("Cursor          {:>5}", size_of::<Cursor>());
    println!("Hit             {:>5}", size_of::<Hit>());
    println!("Stamp           {:>5}", size_of::<Stamp>());
    println!();
    println!("per voxel of a room");
    println!("  core          2 bytes (material, link bits)");
    println!("  light         2 bytes (lamp, sky)");
    println!("  overlay       1 bit, and a hash-map entry per stamped voxel");
    println!();
    println!("per seam");
    println!(
        "  {} bytes for the record and 4 for its entry in the room's list,",
        size_of::<Seam>()
    );
    println!("  whatever the size of the rectangle of faces it covers");
}
