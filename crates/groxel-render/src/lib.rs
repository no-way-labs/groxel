//! Turns rays into pixels.
//!
//! One primary ray per sample and one shadow ray per lit hit, both through
//! the core's tracer. Shadow rays follow seams as sight does, so a shadow can
//! fall through a doorway into a room that is somewhere else.
//!
//! This is the reference renderer: it runs on the CPU, uses every core, and
//! has no dependencies beyond the groxel crates.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod png;
mod shade;

pub use png::write_png;

use groxel::{Cell, EntityId, Overlay, RoomId, Space, Vec3, World};
use groxel_light::Light;
use std::sync::Mutex;

/// How a material's colour varies over a surface.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Pattern {
    /// One colour, with grain per voxel.
    #[default]
    Plain,
    /// Courses of bricks with mortar lines.
    Brick,
    /// Square tiles with joints.
    Tile,
    /// Two colours in alternate blocks.
    Checker,
    /// Long boards.
    Plank,
}

/// How a material looks.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Material {
    /// Its colour, linear RGB.
    pub color: [f32; 3],
    /// The second colour of a [`Pattern::Checker`].
    pub color2: [f32; 3],
    /// Light it gives off. A glowing material is drawn at `color * glow` and
    /// is not shaded.
    pub glow: f32,
    /// How the colour varies.
    pub pattern: Pattern,
    /// Size of one block of the pattern, in voxels.
    pub scale: i32,
    /// Brightness jitter, per voxel for a plain material and per block for a
    /// patterned one.
    pub grain: f32,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            color: [0.70, 0.68, 0.64],
            color2: [0.30, 0.29, 0.28],
            glow: 0.0,
            pattern: Pattern::Plain,
            scale: 4,
            grain: 0.05,
        }
    }
}

/// How a room is lit. Directions are in the room's own frame, so a room
/// looks the same whichever way round you arrive in it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Env {
    /// Sky colour straight up.
    pub zenith: [f32; 3],
    /// Sky colour at the horizon.
    pub horizon: [f32; 3],
    /// Sky colour straight down.
    pub nadir: [f32; 3],
    /// What distance fades to.
    pub haze: [f32; 3],
    /// Unit vector towards the sun.
    pub sun_dir: [f32; 3],
    /// Colour of sunlight.
    pub sun: [f32; 3],
    /// Ambient light on a face that looks up.
    pub sky_light: [f32; 3],
    /// Ambient light on a face that looks down.
    pub ground_light: [f32; 3],
    /// Colour of light from glowing voxels at full strength.
    pub lamp: [f32; 3],
    /// Light that is there regardless, so nothing is pure black.
    pub floor: f32,
    /// Extinction per voxel of distance. 0 turns fog off.
    pub fog: f32,
}

impl Default for Env {
    fn default() -> Self {
        Env {
            zenith: [0.36, 0.52, 0.80],
            horizon: [0.78, 0.84, 0.90],
            nadir: [0.62, 0.68, 0.74],
            haze: [0.78, 0.84, 0.90],
            sun_dir: [0.48, 0.74, 0.47],
            sun: [1.00, 0.94, 0.82],
            sky_light: [0.42, 0.50, 0.62],
            ground_light: [0.30, 0.27, 0.24],
            lamp: [1.25, 0.80, 0.42],
            floor: 0.025,
            fog: 0.006,
        }
    }
}

/// What a picture is made with.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Settings {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Horizontal field of view, in degrees.
    pub fov: f32,
    /// Samples per pixel along each axis.
    pub samples: u32,
    /// Trace a ray to the sun from each lit hit.
    pub shadows: bool,
    /// Darken corners.
    pub ao: bool,
    /// Darkness of the outline drawn on exposed edges. 0 turns it off.
    pub ink: f32,
    /// How far a ray goes.
    pub max_t: f32,
    /// Brightness multiplier.
    pub exposure: f32,
    /// Colour levels per channel, with ordered dither. 0 for full depth.
    pub levels: u32,
    /// Worker threads. 0 uses every core.
    pub threads: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            width: 640,
            height: 360,
            fov: 80.0,
            samples: 1,
            shadows: true,
            ao: true,
            ink: 0.5,
            max_t: 360.0,
            exposure: 1.0,
            levels: 0,
            threads: 0,
        }
    }
}

/// Where the eye is and which way it faces, in the frame of the room the eye
/// is in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Camera {
    /// The voxel the eye is in.
    pub cell: Cell,
    /// The eye.
    pub pos: Vec3,
    /// Unit vector to the right of the view.
    pub right: Vec3,
    /// Unit vector up the view.
    pub up: Vec3,
    /// Unit vector into the view.
    pub fwd: Vec3,
}

/// Everything about one moment of a world that a picture needs. All of it is
/// borrowed and none of it is changed, so several pictures can be made from
/// one frame at once.
#[derive(Copy, Clone)]
pub struct Frame<'a> {
    /// The world.
    pub world: &'a World,
    /// This frame's moving voxels.
    pub overlay: Option<&'a Overlay>,
    /// The world's flooded light.
    pub light: &'a Light,
    /// The entity whose eye this is. Its own voxels are not drawn unless
    /// seen through a seam.
    pub viewer: Option<EntityId>,
}

impl Space for Frame<'_> {
    fn world(&self) -> &World {
        self.world
    }

    fn overlay(&self) -> Option<&Overlay> {
        self.overlay
    }
}

/// The renderer: settings, materials and each room's lighting.
#[derive(Clone, Debug)]
pub struct Renderer {
    /// What pictures are made with.
    pub settings: Settings,
    materials: Vec<Material>,
    envs: Vec<Env>,
    default_env: Env,
}

impl Renderer {
    /// A renderer in which every material and every room has the defaults.
    pub fn new(settings: Settings) -> Renderer {
        Renderer {
            settings,
            materials: vec![Material::default(); 256],
            envs: Vec::new(),
            default_env: Env::default(),
        }
    }

    /// How a material looks.
    pub fn material(&self, id: u8) -> &Material {
        &self.materials[id as usize]
    }

    /// Change how a material looks.
    pub fn set_material(&mut self, id: u8, material: Material) {
        self.materials[id as usize] = material;
    }

    /// How a room is lit.
    pub fn env(&self, room: RoomId) -> &Env {
        self.envs.get(room.0 as usize).unwrap_or(&self.default_env)
    }

    /// Change how a room is lit.
    pub fn set_env(&mut self, room: RoomId, env: Env) {
        let i = room.0 as usize;
        if self.envs.len() <= i {
            self.envs.resize(i + 1, self.default_env);
        }
        self.envs[i] = env;
    }

    /// Fill `out` with the view: `width * height` pixels, row by row from
    /// the top, each `0xAABBGGRR` (bytes R, G, B, A in memory order on a
    /// little-endian machine).
    ///
    /// # Panics
    /// If `out` is not `width * height` long.
    pub fn render(&self, frame: &Frame, camera: &Camera, out: &mut [u32]) {
        let s = &self.settings;
        assert_eq!(
            out.len(),
            s.width as usize * s.height as usize,
            "the buffer must hold width * height pixels"
        );
        let tan_x = (s.fov * std::f32::consts::PI / 360.0).tan();
        let tan_y = tan_x * s.height as f32 / s.width as f32;
        let n = s.samples.max(1);
        let nf = n as f32;
        let (wf, hf) = (s.width as f32, s.height as f32);
        let footprint = 2.0 * tan_x / wf / nf;

        let row = |y: usize, pixels: &mut [u32]| {
            for (x, pixel) in pixels.iter_mut().enumerate() {
                let mut acc = [0.0f32; 3];
                for sy in 0..n {
                    for sx in 0..n {
                        let px = x as f32 + (sx as f32 + 0.5) / nf;
                        let py = y as f32 + (sy as f32 + 0.5) / nf;
                        let kx = (px / wf * 2.0 - 1.0) * tan_x;
                        let ky = (1.0 - py / hf * 2.0) * tan_y;
                        let dir = (camera.fwd + camera.right * kx + camera.up * ky).normalized();
                        let c = self.shade(frame, camera.cell, camera.pos, dir, footprint);
                        for i in 0..3 {
                            acc[i] += c[i];
                        }
                    }
                }
                let k = s.exposure / (nf * nf);
                *pixel = shade::pack(
                    [acc[0] * k, acc[1] * k, acc[2] * k],
                    x as u32,
                    y as u32,
                    s.levels,
                );
            }
        };

        let threads = match s.threads {
            0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
            n => n as usize,
        };
        let rows = out.chunks_mut(s.width as usize).enumerate();
        if threads <= 1 {
            for (y, pixels) in rows {
                row(y, pixels);
            }
            return;
        }
        // Workers take the next row from a shared queue until none is left.
        let queue = Mutex::new(rows);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        let next = queue.lock().expect("no worker panicked").next();
                        let Some((y, pixels)) = next else { break };
                        row(y, pixels);
                    }
                });
            }
        });
    }
}
