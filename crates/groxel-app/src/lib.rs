//! The groxel demo: four worlds that cannot be built, and the figures that
//! walk them.
//!
//! - [`gallery`]: an ordinary room with three doors and a mirror.
//! - [`well`]: a square stairwell whose top opens onto its own bottom.
//! - [`tower`]: a walkway that goes down all the way round a tower and
//!   arrives where it began.
//! - [`hall`]: a corridor whose far end opens onto its near end a quarter
//!   turn round.
//!
//! [`Demo::new`] builds all four, glues the gallery's doors to the other
//! three, and floods the light. From then on the demo is a world to walk a
//! [`groxel_body::Body`] in, a set of walkers to [`tick`](Demo::tick), and a
//! frame to draw:
//!
//! ```
//! use groxel_app::{Demo, camera};
//! use groxel_render::{Renderer, Settings};
//!
//! let mut demo = Demo::new()?;
//! let spot = demo.spot("hall").expect("the hall has a spot");
//! let me = Demo::body_at(&spot);
//! demo.tick(1.0 / 60.0);
//!
//! let settings = Settings { width: 32, height: 18, ..Default::default() };
//! let mut renderer = Renderer::new(settings);
//! demo.paint(&mut renderer);
//!
//! // The figures go into an overlay; the world itself is not changed.
//! let stamped = demo.stamp(Some(&me), 0.0, true);
//! let mut pixels = vec![0; 32 * 18];
//! renderer.render(&demo.frame(&stamped), &camera(&me.view(&demo.world)), &mut pixels);
//! # Ok::<(), groxel::SeamError>(())
//! ```
//!
//! The `still` program in this crate does the same from the command line and
//! writes a PNG. [`shot::Shot`] holds what it can be told.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod gallery;
pub mod hall;
pub mod kit;
pub mod mat;
pub mod shot;
pub mod tower;
pub mod well;

mod demo;
mod figures;
mod palette;

pub use demo::{Demo, Spot, Stamped, VIEWER, Walker, camera};
pub use figures::{monk, put_body, you};
pub use palette::{glows, lin, norm, palette};
