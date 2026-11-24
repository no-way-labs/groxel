//! Something that walks: a box with feet, gravity and an idea of which way
//! is up.
//!
//! A body's position is in one room's frame. When its middle crosses a seam
//! it is carried over, and its own frame turns with it. So "down" is whatever
//! the path that brought the body here says it is: walk the length of a
//! corridor glued to itself a quarter turn round and you are standing on
//! what was the wall.
//!
//! The body is deliberately simple. A project with its own physics can leave
//! this crate out and move a [`groxel::Pose`] instead.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use groxel::{Cell, Face, IVec3, Orientation, RoomId, SeamKind, Vec3, World, vec3};

/// What the body is asked to do this step.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Input {
    /// Forward (positive) or back, from -1 to 1.
    pub forward: f32,
    /// Sideways, from -1 to 1.
    pub strafe: f32,
    /// Fly mode only: straight up or down.
    pub rise: f32,
    /// Change of heading, in radians.
    pub turn: f32,
    /// Change of pitch, in radians.
    pub look: f32,
    /// Jump, if standing.
    pub jump: bool,
    /// Move at running speed.
    pub run: bool,
}

/// What a camera needs: where the eye is and which way it faces, in the
/// frame of the room the eye is in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct View {
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

/// A walking body.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Body {
    /// The room its position is in.
    pub room: RoomId,
    /// The centre of the feet, in the room's frame.
    pub pos: Vec3,
    /// The body's axes (x to its right at heading 0, y up, z forward at
    /// heading 0) to the room's axes.
    pub frame: Orientation,
    /// Heading, in radians.
    pub yaw: f32,
    /// Pitch of the eye, in radians.
    pub pitch: f32,

    /// Half the width of the box.
    pub half_w: f32,
    /// Height of the box.
    pub height: f32,
    /// Height of the eye above the feet.
    pub eye: f32,
    /// Walking speed, voxels a second.
    pub walk_speed: f32,
    /// Running speed.
    pub run_speed: f32,
    /// Upward speed at the start of a jump.
    pub jump_speed: f32,
    /// Downward acceleration.
    pub gravity: f32,
    /// The tallest step it climbs without jumping.
    pub step_up: f32,

    /// Speed along its own up axis.
    pub vel_up: f32,
    /// True if it is standing on something.
    pub on_ground: bool,
    /// No gravity; moves where it looks.
    pub fly: bool,
    /// Passes through solid voxels (a film camera). Still follows seams.
    pub ghost: bool,
    /// Eases the eye over steps so that stairs do not judder.
    pub eye_lag: f32,
    /// Seams crossed so far.
    pub crossings: u32,
    /// Where the head pointed, in the room's frame, before the body was last
    /// stood upright by [`stand_up`](Body::stand_up).
    pub tilt_from: Vec3,
    /// How far the body's middle was from where it is now, before it was
    /// stood upright: zero unless it had to be lifted clear of a floor.
    pub tilt_slide: Vec3,
    /// How much of that turn the eye has still to make, from 1 down to 0.
    /// The body turns at once; the eye follows over a moment, so the view
    /// rolls and does not jump.
    pub tilt: f32,
}

/// Seconds the eye takes to follow a body that has been stood upright.
const STAND_UP_TIME: f32 = 0.45;

impl Body {
    /// A body with its feet at `pos`, upright in the room's frame.
    pub fn new(room: RoomId, pos: Vec3) -> Body {
        Body {
            room,
            pos,
            frame: Orientation::IDENTITY,
            yaw: 0.0,
            pitch: 0.0,
            half_w: 0.9,
            height: 6.6,
            eye: 6.0,
            walk_speed: 10.0,
            run_speed: 20.0,
            jump_speed: 15.0,
            gravity: 42.0,
            step_up: 1.05,
            vel_up: 0.0,
            on_ground: false,
            fly: false,
            ghost: false,
            eye_lag: 0.0,
            crossings: 0,
            tilt_from: Vec3::ZERO,
            tilt_slide: Vec3::ZERO,
            tilt: 0.0,
        }
    }

    /// Which face of the room the body's head points at.
    pub fn up_face(&self) -> Face {
        self.frame.face(Face::PosY)
    }

    fn anchor_of(&self, pos: Vec3) -> Vec3 {
        let u = self.up_face();
        let mut a = pos;
        a[u.axis()] += u.sign() as f32 * self.height * 0.5;
        a
    }

    /// The voxel holding the body's middle. Seams are followed when this
    /// changes, and everything else about the body is found relative to it.
    pub fn anchor(&self) -> Cell {
        Cell {
            room: self.room,
            at: self.anchor_of(self.pos).floor(),
        }
    }

    fn blocked(&self, world: &World, pos: Vec3) -> bool {
        let from = self.anchor();
        let to = self.anchor_of(pos).floor();
        let room = world.room(self.room);
        for i in 0..3 {
            if to[i] == from.at[i] {
                continue;
            }
            match world.seam_at(from, Face::new(i, to[i] > from.at[i])) {
                Some(id) if world.seam(id).kind == SeamKind::SightOnly => return true,
                Some(_) => {}
                None if !room.contains(to) => return true,
                None => {}
            }
        }
        if self.ghost {
            return false;
        }

        let u = self.up_face();
        let mut lo = pos - vec3(self.half_w, self.half_w, self.half_w);
        let mut hi = pos + vec3(self.half_w, self.half_w, self.half_w);
        if u.sign() > 0 {
            lo[u.axis()] = pos[u.axis()];
            hi[u.axis()] = pos[u.axis()] + self.height;
        } else {
            lo[u.axis()] = pos[u.axis()] - self.height;
            hi[u.axis()] = pos[u.axis()];
        }
        let eps = vec3(1e-3, 1e-3, 1e-3);
        let (c0, c1) = ((lo + eps).floor(), (hi - eps).floor());
        for z in c0[2]..=c1[2] {
            for y in c0[1]..=c1[1] {
                for x in c0[0]..=c1[0] {
                    // Each voxel of the box is found by walking from the
                    // middle, so a box that straddles a seam is tested
                    // against what is really on the other side. A mirror
                    // is a wall: the walk does not pass it.
                    let Some((there, _)) = world.walk_open(from, IVec3([x, y, z]) - from.at) else {
                        return true;
                    };
                    if world.room(there.room).is_solid(there.at) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Move along one axis of the room. Returns how far it got, with the
    /// sign it was asked for. The axis is re-read whenever a seam turns the
    /// body on the way.
    fn move_axis(&mut self, world: &World, axis: usize, amount: f32) -> f32 {
        let mut axis = axis;
        let mut remaining = amount;
        let mut sign = 1.0;
        let mut done = 0.0;
        while remaining != 0.0 {
            let mut s = remaining.clamp(-0.45, 0.45);
            let step_to = |s: f32, pos: Vec3| {
                let mut np = pos;
                np[axis] += s * sign;
                np
            };
            // Blocked: close the gap by halves, so the body rests against
            // things.
            let mut np = step_to(s, self.pos);
            let mut free = !self.blocked(world, np);
            while !free && s.abs() > 1.0 / 128.0 {
                s *= 0.5;
                np = step_to(s, self.pos);
                free = !self.blocked(world, np);
                remaining = s;
            }
            if !free {
                break;
            }

            let from = self.anchor();
            self.pos = np;
            let to = self.anchor().at;
            if to[axis] != from.at[axis] {
                let face = Face::new(axis, to[axis] > from.at[axis]);
                if let Some(id) = world.seam_at(from, face) {
                    let seam = world.seam(id);
                    let turn = seam.map.rot;
                    // Carried over relative to the middle's voxel, which
                    // keeps the arithmetic small.
                    let rel = turn.apply_f(self.pos - to.centre());
                    self.pos = rel + seam.map.cell(to).centre();
                    self.frame = self.frame.then(turn);
                    self.room = seam.to;
                    self.crossings += 1;
                    self.tilt_from = turn.apply_f(self.tilt_from);
                    self.tilt_slide = turn.apply_f(self.tilt_slide);
                    let new_axis = turn.axis_to(axis);
                    if turn.source(new_axis).1 < 0 {
                        sign = -sign;
                    }
                    axis = new_axis;
                }
            }
            done += s;
            remaining -= s;
        }
        done
    }

    /// Move along one of the body's own axes: 0 to its right at heading 0,
    /// 1 up, 2 forward at heading 0.
    fn move_local(&mut self, world: &World, local_axis: usize, amount: f32) -> f32 {
        let j = self.frame.axis_to(local_axis);
        let s = self.frame.source(j).1 as f32;
        self.move_axis(world, j, amount * s) * s
    }

    fn snap_to_ground(&mut self) {
        let u = self.up_face();
        let a = u.axis();
        self.pos[a] = if u.sign() > 0 {
            (self.pos[a] + 0.01).floor()
        } else {
            (self.pos[a] - 0.01).ceil()
        };
    }

    /// Advance the body by `dt` seconds.
    pub fn update(&mut self, world: &World, input: &Input, dt: f32) {
        self.yaw += input.turn;
        self.pitch = (self.pitch + input.look).clamp(-1.5, 1.5);
        self.eye_lag *= (-dt * 10.0).exp();
        self.tilt = (self.tilt - dt / STAND_UP_TIME).max(0.0);

        let (sy, cy) = self.yaw.sin_cos();
        let speed = if input.run {
            self.run_speed
        } else {
            self.walk_speed
        } * dt;

        if self.fly {
            let (sp, cp) = self.pitch.sin_cos();
            let mx = (sy * cp * input.forward - cy * input.strafe) * speed;
            let my = (sp * input.forward + input.rise) * speed;
            let mz = (cy * cp * input.forward + sy * input.strafe) * speed;
            self.move_local(world, 0, mx);
            self.move_local(world, 1, my);
            self.move_local(world, 2, mz);
            self.vel_up = 0.0;
            self.on_ground = false;
            return;
        }

        let (mut f, mut s) = (input.forward, input.strafe);
        let len = (f * f + s * s).sqrt();
        if len > 1.0 {
            f /= len;
            s /= len;
        }
        let wish = [
            (0, (sy * f - cy * s) * speed),
            (2, (cy * f + sy * s) * speed),
        ];
        let was_ground = self.on_ground;

        for (axis, amount) in wish {
            if amount == 0.0 {
                continue;
            }
            let got = self.move_local(world, axis, amount);
            let left = amount - got;
            if left.abs() < 1e-4 || !was_ground {
                continue;
            }
            // Blocked: is it a step we can climb?
            let saved = *self;
            let rose = self.move_local(world, 1, self.step_up);
            if rose < self.step_up - 1e-3 || self.move_local(world, axis, left).abs() < 1e-4 {
                *self = saved;
            } else {
                self.eye_lag -= rose;
            }
        }

        if input.jump && self.on_ground {
            self.vel_up = self.jump_speed;
            self.on_ground = false;
        } else if was_ground && self.vel_up <= 0.0 {
            // Keep the feet on descending stairs instead of hopping off each
            // one.
            let u = self.up_face();
            let mut probe = self.pos;
            probe[u.axis()] -= u.sign() as f32 * self.step_up;
            if self.blocked(world, probe) {
                let dropped = -self.move_local(world, 1, -self.step_up);
                let a = self.up_face().axis();
                let before = self.pos[a];
                self.snap_to_ground();
                let total = dropped + (self.pos[a] - before).abs();
                if total > 0.02 {
                    self.eye_lag += total;
                }
                self.vel_up = 0.0;
                self.on_ground = true;
                return;
            }
        }

        self.vel_up -= self.gravity * dt;
        let dy = self.vel_up * dt;
        let moved = self.move_local(world, 1, dy);
        if (moved - dy).abs() > 1e-5 {
            if dy < 0.0 {
                self.snap_to_ground();
                self.on_ground = true;
            }
            self.vel_up = 0.0;
        } else {
            self.on_ground = false;
        }
    }

    /// Which way the head points, as a vector in the room's frame.
    fn up_vec(&self) -> Vec3 {
        let n = self.up_face().normal();
        vec3(n[0] as f32, n[1] as f32, n[2] as f32)
    }

    /// Stand the body upright in a room whose "up" is the face `up`.
    ///
    /// A body's frame is turned only by the seams it crosses, so a body that
    /// comes out of a corridor that turned it is lying on its side in the
    /// next room and falls towards a wall. A world that wants its rooms to
    /// have a floor calls this for a body that arrives turned. The body
    /// turns about its middle, by the quarter or half turn that takes its
    /// head to `up`, and goes on looking the way it was looking. If its
    /// feet would then be in the floor it is lifted until they are clear.
    /// Its eye follows over a moment (see [`tilt`](Body::tilt)).
    ///
    /// Returns false, and changes nothing, if there is no room to turn where
    /// the body is. Calling it again each step turns the body as soon as
    /// there is. Returns true if the body is upright already.
    pub fn stand_up(&mut self, world: &World, up: Face) -> bool {
        let head = self.up_face();
        if head == up {
            return true;
        }
        // The rotation that takes the head to `up` and leaves the axis
        // across both where it is. For a half turn any axis across will do.
        let across = head.cross(up);
        let turn = Orientation::all()
            .filter(|r| !r.is_reflection() && r.face(head) == up)
            .find(|r| across.is_none_or(|axis| r.face(axis) == axis))
            .expect("some rotation takes any face to any other");

        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let look = self.frame.apply_f(vec3(sy * cp, sp, cy * cp));
        let middle = self.anchor_of(self.pos);

        let mut turned = *self;
        turned.frame = self.frame.then(turn);
        let rise = turned.up_vec();
        turned.pos = middle - rise * (self.height * 0.5);
        // Turned about its middle, or failing that lifted by half a voxel
        // at a time until it fits, for as far as it is tall.
        let room = world.room(self.room);
        let Some(feet) = (0..=(2.0 * self.height) as i32)
            .map(|k| turned.pos + rise * (k as f32 * 0.5))
            .find(|&p| room.contains(turned.anchor_of(p).floor()) && !turned.blocked(world, p))
        else {
            return false;
        };
        turned.pos = feet;
        // Keep looking the same way, as far as the new frame allows.
        let local = turned.frame.inverse().apply_f(look);
        turned.pitch = local[1].clamp(-1.0, 1.0).asin().clamp(-1.5, 1.5);
        if local[0].abs() + local[2].abs() > 1e-4 {
            turned.yaw = local[0].atan2(local[2]);
        }
        turned.vel_up = 0.0;
        turned.on_ground = false;
        turned.tilt_from = self.up_vec();
        turned.tilt_slide = middle - turned.anchor_of(feet);
        turned.tilt = 1.0;
        *self = turned;
        true
    }

    /// The eye, resolved through any seam that lies between it and the
    /// body's middle: a head through a doorway, or over a wrap.
    pub fn view(&self, world: &World) -> View {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let fwd_l = vec3(sy * cp, sp, cy * cp);
        let right_l = vec3(-cy, 0.0, sy);
        let fwd = self.frame.apply_f(fwd_l);
        let mut right = self.frame.apply_f(right_l);
        let mut up = self.frame.apply_f(right_l.cross(fwd_l));

        let u = self.up_face();
        let mut eye = self.pos;
        eye[u.axis()] += u.sign() as f32 * (self.eye + self.eye_lag);

        if self.tilt > 0.0 {
            // Still being stood upright: the eye is where a head part-way
            // between the old "up" and the new one would have it.
            let k = self.tilt * self.tilt * (3.0 - 2.0 * self.tilt);
            let head = self.up_vec() * (1.0 - k) + self.tilt_from * k;
            let side = fwd.cross(head);
            if head.length() > 1e-3 && side.length() > 1e-3 {
                let head = head.normalized();
                let handed = if self.frame.is_reflection() {
                    -1.0
                } else {
                    1.0
                };
                right = side.normalized() * handed;
                up = right.cross(fwd) * handed;
                let reach = self.eye + self.eye_lag - self.height * 0.5;
                eye = self.anchor_of(self.pos) + self.tilt_slide * k + head * reach;
            }
        }

        let from = self.anchor();
        let at = eye.floor();
        match world.walk_open(from, at - from.at) {
            Some((there, turn)) => View {
                cell: there,
                pos: turn.apply_f(eye - at.centre()) + there.at.centre(),
                right: turn.apply_f(right),
                up: turn.apply_f(up),
                fwd: turn.apply_f(fwd),
            },
            None => View {
                cell: from,
                pos: self.anchor_of(self.pos),
                right,
                up,
                fwd,
            },
        }
    }

    /// The heading that faces a point given in the current room's frame,
    /// ignoring height.
    pub fn yaw_toward(&self, target: Vec3) -> f32 {
        let d = self.frame.inverse().apply_f(target - self.pos);
        d[0].atan2(d[2])
    }
}
