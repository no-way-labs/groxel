//! The demo: four worlds glued together, their light, and their walkers.

use crate::figures::{monk, put_body, you};
use crate::palette::{glows, palette};
use crate::{gallery, hall, tower, well};
use groxel::{EntityId, Face, IVec3, Orientation, Overlay, RoomId, SeamError, Vec3, World, vec3};
use groxel_body::{Body, Input, View};
use groxel_light::Light;
use groxel_render::{Camera, Env, Frame, Renderer};

/// The entity id of the viewer's own figure in a frame's overlay. Walker
/// number `i` is `EntityId(i + 1)`.
pub const VIEWER: EntityId = EntityId(0);

/// A walker inside this distance of the viewer, in the same room, is left
/// out of the frame.
const ARMS_REACH: f32 = 4.5;

/// How far a figure walks on one frame of its stride.
const STRIDE: f32 = 2.5;

/// A figure that walks a closed circuit of points, forever.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Walker {
    /// The body that does the walking.
    pub body: Body,
    /// The circuit, as (x, z) points in the frame of whichever room the
    /// walker is in. A walker with no points keeps going the way it faces.
    pub path: &'static [[f32; 2]],
    /// The index of the point it is walking towards.
    pub next: usize,
    /// How far it has walked, which says where it is in its stride.
    pub odometer: f32,
}

/// A named place to stand.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Spot {
    /// What it is called.
    pub name: &'static str,
    /// The room it is in.
    pub room: RoomId,
    /// Where the feet go, in the room's frame.
    pub pos: Vec3,
    /// Heading, in radians.
    pub yaw: f32,
    /// Pitch of the eye, in radians.
    pub pitch: f32,
    /// The body's axes to the room's axes.
    pub frame: Orientation,
    /// True for a place in mid air: a body put here flies.
    pub fly: bool,
}

/// One frame's moving voxels: every walker's figure and, if it is shown,
/// the viewer's. [`Demo::frame`] turns it into what the renderer takes.
#[derive(Clone, Debug)]
pub struct Stamped {
    /// The figures, as voxels laid over the world.
    pub overlay: Overlay,
    /// The viewer's entity, if the viewer's figure is in the overlay. The
    /// renderer leaves its voxels out unless they are seen through a seam.
    pub viewer: Option<EntityId>,
}

/// The four impossible worlds and the figures that walk them.
#[derive(Clone, Debug)]
pub struct Demo {
    /// The gallery, the well, the tower and the hall, glued together.
    pub world: World,
    /// The world's flooded light, as of the last [`relight`](Demo::relight).
    pub light: Light,
    /// The figures that walk the worlds.
    pub walkers: Vec<Walker>,
    /// Named places to stand, in the order they were made.
    pub spots: Vec<Spot>,
    /// How each room is lit, by room number.
    envs: Vec<Env>,
    /// Which rooms are open air, by room number.
    outdoors: Vec<bool>,
    /// Which way is up in each room, by room number, for rooms that have a
    /// floor. `None` for a room in which up is wherever you arrive with it.
    ups: Vec<Option<Face>>,
}

impl Demo {
    /// Build the gallery, the well, the tower and the hall, glue them
    /// together, and flood their light.
    pub fn new() -> Result<Demo, SeamError> {
        let world = World::new();
        let light = Light::flood(&world, glows, |_| false);
        let mut demo = Demo {
            world,
            light,
            walkers: Vec::new(),
            spots: Vec::new(),
            envs: Vec::new(),
            outdoors: Vec::new(),
            ups: Vec::new(),
        };
        let gallery = gallery::build(&mut demo)?;
        well::build(&mut demo, gallery)?;
        tower::build(&mut demo, gallery)?;
        hall::build(&mut demo, gallery)?;
        demo.relight();
        Ok(demo)
    }

    /// Work the flooded light out again. Call this after changing voxels.
    pub fn relight(&mut self) {
        let outdoors = &self.outdoors;
        self.light = Light::flood(&self.world, glows, |room| {
            outdoors.get(room.0 as usize).copied().unwrap_or(false)
        });
    }

    /// The spot with this name, if there is one.
    pub fn spot(&self, name: &str) -> Option<Spot> {
        self.spots.iter().find(|spot| spot.name == name).copied()
    }

    /// A body standing on a spot, facing the way the spot faces.
    pub fn body_at(spot: &Spot) -> Body {
        let mut body = Body::new(spot.room, spot.pos);
        body.yaw = spot.yaw;
        body.pitch = spot.pitch;
        body.frame = spot.frame;
        body.fly = spot.fly;
        body
    }

    /// How a room is lit.
    pub fn env(&self, room: RoomId) -> Env {
        self.envs.get(room.0 as usize).copied().unwrap_or_default()
    }

    /// Which way is up in a room, if the room says. The gallery, the well
    /// and the tower have floors. The hall does not: it is the room that
    /// turns you.
    pub fn up(&self, room: RoomId) -> Option<Face> {
        self.ups.get(room.0 as usize).copied().flatten()
    }

    /// Stand a body upright if it is in a room with a floor and is not
    /// upright there. A body's own "up" is turned only by the seams it
    /// crosses, so one that leaves the hall on its side would otherwise fall
    /// towards a wall of the next room and live on it. Call this after each
    /// step of a body. Returns false while the body has no room to turn.
    pub fn right(&self, body: &mut Body) -> bool {
        match self.up(body.room) {
            Some(up) => body.stand_up(&self.world, up),
            None => true,
        }
    }

    /// True if a room is open air, where every empty voxel sees the sky.
    pub fn is_outdoors(&self, room: RoomId) -> bool {
        self.outdoors.get(room.0 as usize).copied().unwrap_or(false)
    }

    /// Give a renderer the palette these worlds are built from, and the
    /// lighting of each room.
    pub fn paint(&self, renderer: &mut Renderer) {
        for (id, material) in palette().iter().enumerate() {
            renderer.set_material(id as u8, *material);
        }
        for (i, env) in self.envs.iter().enumerate() {
            renderer.set_env(RoomId(i as u32), *env);
        }
    }

    /// Advance the walkers by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        let forward = Input {
            forward: 1.0,
            ..Default::default()
        };
        for k in &mut self.walkers {
            // With no path, a walker just keeps going the way it faces.
            if !k.path.is_empty() {
                // Follow the line from the last corner to the next, aiming a
                // little way ahead along it, so corners are not cut.
                let n = k.path.len();
                let from = k.path[(k.next + n - 1) % n];
                let to = k.path[k.next];
                let sx = to[0] - from[0];
                let sz = to[1] - from[1];
                let len = (sx * sx + sz * sz).sqrt();
                let along = ((k.body.pos[0] - from[0]) * sx + (k.body.pos[2] - from[1]) * sz) / len;
                if along > len - 0.5 {
                    k.next = (k.next + 1) % n;
                }
                let ahead = (along + 3.0).clamp(0.0, len) / len;
                k.body.yaw = k.body.yaw_toward(vec3(
                    from[0] + sx * ahead,
                    k.body.pos[1],
                    from[1] + sz * ahead,
                ));
            }
            k.body.update(&self.world, &forward, dt);
            k.odometer += k.body.walk_speed * dt;
        }
    }

    /// Make this frame's overlay: every walker's figure, and the viewer's.
    ///
    /// `viewer`, if given, is whoever is looking. A walker within arm's
    /// reach of the viewer is left out, because a figure walking through the
    /// camera fills the screen. With `show_viewer` the viewer's own figure
    /// is put in too; the renderer then draws it only where it is seen
    /// through a seam. `viewer_odometer` is how far the viewer has walked,
    /// which says where it is in its stride.
    ///
    /// The world is not changed.
    pub fn stamp(&self, viewer: Option<&Body>, viewer_odometer: f32, show_viewer: bool) -> Stamped {
        let mut overlay = Overlay::new(&self.world);
        for (i, k) in self.walkers.iter().enumerate() {
            if viewer.is_some_and(|b| within_reach(b, &k.body)) {
                continue;
            }
            let sprite = &monk()[stride(k.odometer)];
            let entity = EntityId(i as u32 + 1);
            put_body(&mut overlay, &self.world, &k.body, sprite, entity);
        }
        let viewer = viewer.filter(|_| show_viewer).map(|b| {
            let sprite = &you()[stride(viewer_odometer)];
            put_body(&mut overlay, &self.world, b, sprite, VIEWER);
            VIEWER
        });
        Stamped { overlay, viewer }
    }

    /// Everything the renderer needs to draw this moment: the world, its
    /// light, and the figures from [`stamp`](Demo::stamp).
    pub fn frame<'a>(&'a self, stamped: &'a Stamped) -> Frame<'a> {
        Frame {
            world: &self.world,
            overlay: Some(&stamped.overlay),
            light: &self.light,
            viewer: stamped.viewer,
        }
    }

    /// Add a room, with its lighting.
    pub(crate) fn add_room(&mut self, name: &str, dims: IVec3, env: Env, outdoors: bool) -> RoomId {
        let id = self.world.add_room(name, dims);
        let i = id.0 as usize;
        if self.envs.len() <= i {
            self.envs.resize(i + 1, Env::default());
            self.outdoors.resize(i + 1, false);
            self.ups.resize(i + 1, Some(Face::PosY));
        }
        self.envs[i] = env;
        self.outdoors[i] = outdoors;
        id
    }

    /// Say that a room has no floor of its own: a body in it keeps the "up"
    /// it came in with.
    pub(crate) fn turns_freely(&mut self, room: RoomId) {
        self.ups[room.0 as usize] = None;
    }

    /// Put `count` walkers on a circuit, spread evenly along it.
    /// `feet_at` gives the height to stand at over a column.
    pub(crate) fn add_walkers(
        &mut self,
        room: RoomId,
        path: &'static [[f32; 2]],
        count: usize,
        feet_at: fn(i32, i32) -> f32,
        speed: f32,
    ) {
        let leg = |i: usize| {
            let (p, q) = (path[i], path[(i + 1) % path.len()]);
            (p, q, (q[0] - p[0]).abs() + (q[1] - p[1]).abs())
        };
        let mut total = 0.0;
        for i in 0..path.len() {
            total += leg(i).2;
        }
        for n in 0..count {
            let mut want = total * n as f32 / count as f32;
            let mut i = 0;
            loop {
                let (p, q, len) = leg(i);
                if want <= len {
                    let t = want / len;
                    let x = p[0] + (q[0] - p[0]) * t;
                    let z = p[1] + (q[1] - p[1]) * t;
                    let y = feet_at(x.floor() as i32, z.floor() as i32);
                    // Start a little high and let it drop: the footprint
                    // may overlap the next step up.
                    let mut body = Body::new(room, vec3(x, y + 1.25, z));
                    body.walk_speed = speed;
                    body.yaw = body.yaw_toward(vec3(q[0], 0.0, q[1]));
                    self.walkers.push(Walker {
                        body,
                        path,
                        next: (i + 1) % path.len(),
                        odometer: 0.0,
                    });
                    break;
                }
                want -= len;
                i = (i + 1) % path.len();
            }
        }
    }
}

/// True if a walker is in the viewer's room and within arm's reach.
fn within_reach(viewer: &Body, walker: &Body) -> bool {
    if viewer.room != walker.room {
        return false;
    }
    let mut d2 = 0.0;
    for i in 0..3 {
        d2 += (viewer.pos[i] - walker.pos[i]) * (viewer.pos[i] - walker.pos[i]);
    }
    d2 < ARMS_REACH * ARMS_REACH
}

/// Which of a figure's two poses to draw after walking this far.
fn stride(odometer: f32) -> usize {
    // The cap is for an odometer a hair below zero, whose remainder rounds
    // up to 2.
    ((odometer / STRIDE).rem_euclid(2.0) as usize).min(1)
}

/// The camera that sees what a body sees.
pub fn camera(view: &View) -> Camera {
    Camera {
        cell: view.cell,
        pos: view.pos,
        right: view.right,
        up: view.up,
        fwd: view.fwd,
    }
}
