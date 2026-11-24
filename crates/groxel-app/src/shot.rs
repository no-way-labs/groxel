//! A headless camera: how a still of the demo worlds is set up and taken.

use crate::demo::{Demo, Spot, camera};
use groxel_body::{Body, Input};
use groxel_render::{Renderer, Settings};
use std::f32::consts::PI;
use std::fmt;

/// How a still is taken: the picture's settings, where the camera stands
/// relative to a spot, and what happens before the picture is made.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shot {
    /// What the picture is made with.
    pub settings: Settings,
    /// How far along x the camera stands from the spot.
    pub dx: f32,
    /// How far along y the camera stands from the spot.
    pub dy: f32,
    /// How far along z the camera stands from the spot.
    pub dz: f32,
    /// A heading in radians, in place of the spot's.
    pub yaw: Option<f32>,
    /// A pitch in radians, in place of the spot's.
    pub pitch: Option<f32>,
    /// Seconds of world time before the shot.
    pub t: f32,
    /// Put the viewer's own figure in the world, to be seen in mirrors and
    /// round loops.
    pub me: bool,
    /// Voxels to walk forward before the shot.
    pub walk: f32,
    /// Make the camera fly, so that it stays where it is put.
    pub fly: bool,
}

impl Default for Shot {
    /// 960 by 540 with four samples a pixel, two seconds after the world
    /// starts, from the spot itself, with the viewer's figure in place.
    fn default() -> Self {
        Shot {
            settings: Settings {
                width: 960,
                height: 540,
                samples: 2,
                ..Default::default()
            },
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            yaw: None,
            pitch: None,
            t: 2.0,
            me: true,
            walk: 0.0,
            fly: false,
        }
    }
}

/// An argument that [`Shot::parse`] could not use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadArgument(pub String);

impl fmt::Display for BadArgument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cannot use '{}': an argument is key=number, where the key is one of {}",
            self.0,
            Shot::KEYS
        )
    }
}

impl std::error::Error for BadArgument {}

impl Shot {
    /// The keys [`parse`](Shot::parse) knows.
    pub const KEYS: &'static str =
        "w h ss fov shadows ao ink levels threads dx dy dz yaw pitch t me walk fly";

    /// Change the shot by a list of `key=value` arguments. Every value is a
    /// number.
    ///
    /// | Key | Sets |
    /// |---|---|
    /// | `w`, `h` | width and height in pixels, each at least 1 |
    /// | `ss` | samples per pixel along each axis |
    /// | `fov` | horizontal field of view, degrees |
    /// | `shadows`, `ao` | sun shadows and corner darkening; 0 turns one off |
    /// | `ink` | darkness of outlines; 0 turns them off |
    /// | `levels` | colour levels per channel; 0 for full depth |
    /// | `threads` | worker threads; 0 for every core |
    /// | `dx`, `dy`, `dz` | where the camera stands, relative to the spot |
    /// | `yaw`, `pitch` | heading and pitch, degrees |
    /// | `t` | seconds of world time before the shot |
    /// | `me` | 0 leaves the viewer's own figure out |
    /// | `walk` | voxels to walk forward first |
    /// | `fly` | anything but 0 makes the camera fly |
    pub fn parse<S: AsRef<str>>(&mut self, args: &[S]) -> Result<(), BadArgument> {
        for arg in args {
            let arg = arg.as_ref();
            let bad = || BadArgument(arg.to_owned());
            let (key, value) = arg.split_once('=').ok_or_else(bad)?;
            let val: f32 = value.parse().map_err(|_| bad())?;
            let s = &mut self.settings;
            match key {
                "w" if val >= 1.0 => s.width = val as u32,
                "h" if val >= 1.0 => s.height = val as u32,
                "ss" => s.samples = val as u32,
                "fov" => s.fov = val,
                "shadows" => s.shadows = val != 0.0,
                "ao" => s.ao = val != 0.0,
                "ink" => s.ink = val,
                "levels" => s.levels = val as u32,
                "threads" => s.threads = val as u32,
                "dx" => self.dx = val,
                "dy" => self.dy = val,
                "dz" => self.dz = val,
                "yaw" => self.yaw = Some(val * PI / 180.0),
                "pitch" => self.pitch = Some(val * PI / 180.0),
                "t" => self.t = val,
                "me" => self.me = val != 0.0,
                "walk" => self.walk = val,
                "fly" => self.fly = val != 0.0,
                _ => return Err(bad()),
            }
        }
        Ok(())
    }

    /// The body as it is first put down: on the spot, moved by `dx`, `dy`
    /// and `dz`, with the shot's heading.
    pub fn start(&self, spot: &Spot) -> Body {
        let mut body = Demo::body_at(spot);
        body.pos[0] += self.dx;
        body.pos[1] += self.dy;
        body.pos[2] += self.dz;
        if let Some(yaw) = self.yaw {
            body.yaw = yaw;
        }
        if self.fly {
            body.fly = true;
        }
        body
    }

    /// Stand a body on a spot, let the world run, and return the body.
    ///
    /// The world is ticked 60 times a second until `t` seconds have passed
    /// and the body has walked `walk` voxels. The clock is a sum of single
    /// precision steps of a sixtieth, which falls a little short of the
    /// whole seconds, so the default two seconds is 121 ticks and three
    /// seconds is 181.
    ///
    /// A body that is not flying falls from where it is put, and takes about
    /// a second to drop 20 voxels. A shot with a short `t` can catch it on
    /// the way down.
    ///
    /// # Panics
    /// If the offsets put the middle of the body outside the spot's room.
    pub fn stage(&self, demo: &mut Demo, spot: &Spot) -> Body {
        let mut body = self.start(spot);
        assert!(
            demo.world.room(body.room).contains(body.anchor().at),
            "the camera must start inside its room"
        );
        let dt: f32 = 1.0 / 60.0;
        let mut t: f32 = 0.0;
        let mut walked: f32 = 0.0;
        while t < self.t || walked < self.walk {
            demo.tick(dt);
            let go = walked < self.walk;
            let input = Input {
                forward: if go { 1.0 } else { 0.0 },
                ..Default::default()
            };
            body.update(&demo.world, &input, dt);
            if go {
                walked += body.walk_speed * dt;
            }
            t += dt;
        }
        // The pitch is set after the wait, and the eye is not left easing
        // over a step.
        if let Some(pitch) = self.pitch {
            body.pitch = pitch;
        }
        body.eye_lag = 0.0;
        body
    }

    /// Draw what a body sees: `width * height` pixels, row by row from the
    /// top, as [`Renderer::render`] makes them.
    pub fn render(&self, demo: &Demo, body: &Body) -> Vec<u32> {
        let s = &self.settings;
        let mut renderer = Renderer::new(*s);
        demo.paint(&mut renderer);
        let stamped = demo.stamp(Some(body), 0.0, self.me);
        let mut pixels = vec![0; s.width as usize * s.height as usize];
        renderer.render(
            &demo.frame(&stamped),
            &camera(&body.view(&demo.world)),
            &mut pixels,
        );
        pixels
    }

    /// Stage the camera on a spot and draw what it sees.
    pub fn take(&self, demo: &mut Demo, spot: &Spot) -> Vec<u32> {
        let body = self.stage(demo, spot);
        self.render(demo, &body)
    }
}
