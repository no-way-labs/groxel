//! The colour along one ray.

use crate::{Env, Frame, Material, Pattern, Renderer};
use groxel::{Cell, Face, Hit, HitKind, IVec3, Orientation, Ray, Skip, TraceOpts, Vec3, trace};
use groxel_light::REACH;

/// How far a ray must have gone before the viewer's own voxels are drawn.
/// It has to cover the figure from its eye: the demo's figure is seven voxels
/// tall and three wide, with the eye six voxels above its feet.
const VIEWER_NEAR: f32 = 7.0;

impl Renderer {
    /// The colour seen along one ray from `origin`, which is in voxel `cell`.
    /// `footprint` is the width of a pixel at distance one, and sets how wide
    /// outlines and mortar lines are drawn.
    pub fn shade(
        &self,
        frame: &Frame,
        cell: Cell,
        origin: Vec3,
        dir: Vec3,
        footprint: f32,
    ) -> [f32; 3] {
        let s = &self.settings;
        let opts = TraceOpts {
            max_t: s.max_t,
            skip: frame.viewer.map(|entity| Skip {
                entity,
                near: VIEWER_NEAR,
            }),
            ..Default::default()
        };
        let ray = Ray {
            start: cell,
            origin,
            dir,
        };
        let h = trace(frame, ray, &opts);
        let env = self.env(h.cell.room);
        match h.kind {
            HitKind::Sky => return sky(env, h.dir),
            HitKind::Far => return env.haze,
            HitKind::Solid => {}
        }

        let m = self.material(h.material);
        let a = h.face.axis();
        let (ua, va) = ((a + 1) % 3, (a + 2) % 3);
        let sgn = h.face.sign() as f32;

        let p = h.point();
        let fu = (p[ua] - h.cell.at[ua] as f32).clamp(0.0, 1.0);
        let fv = (p[va] - h.cell.at[va] as f32).clamp(0.0, 1.0);
        let size = footprint * h.t;

        // What surrounds the empty voxel in front of the face (for occlusion,
        // creases and flooded light), and what continues the face sideways
        // (for outlines).
        let ring = Ring::sample(frame, &h, ua, va);
        let side = [
            solid_across(frame, h.cell, Face::new(ua, false)),
            solid_across(frame, h.cell, Face::new(ua, true)),
            solid_across(frame, h.cell, Face::new(va, false)),
            solid_across(frame, h.cell, Face::new(va, true)),
        ];
        let above = [
            ring.solid[0][1],
            ring.solid[2][1],
            ring.solid[1][0],
            ring.solid[1][2],
        ];
        let edge = [fu, 1.0 - fu, fv, 1.0 - fv];

        let mut col = albedo(m, h.cell.at, a, edge, size);

        if m.glow > 0.0 {
            for c in &mut col {
                *c *= m.glow;
            }
        } else {
            // Bilinear weights of the face's four corners.
            let wt = [
                (1.0 - fu) * (1.0 - fv),
                fu * (1.0 - fv),
                (1.0 - fu) * fv,
                fu * fv,
            ];
            let (mut ao, mut lamp, mut skyl) = (0.0, 0.0, 0.0);
            for (k, w) in wt.iter().enumerate() {
                let c = ring.corner((k & 1) * 2, (k >> 1) * 2);
                ao += w * if s.ao { c.ao } else { 1.0 };
                lamp += w * c.lamp;
                skyl += w * c.sky;
            }
            lamp = lamp * lamp * lamp * Ring::facing(ring.lamp[1][1], ring.lamp_out);
            skyl = skyl * skyl * skyl;
            let sky_facing = Ring::facing(ring.sky[1][1], ring.sky_out);

            let sun_n = env.sun_dir[a] * sgn;
            let mut lit = sun_n.max(0.0) * (skyl * 4.0).min(1.0);
            if lit > 0.001 && s.shadows && in_shadow(frame, &h, p, env.sun_dir) {
                lit = 0.0;
            }
            let up_n = if a == 1 { sgn } else { 0.0 };
            let hemi = up_n * 0.5 + 0.5;
            for (i, c) in col.iter_mut().enumerate() {
                let ambient = (env.ground_light[i]
                    + (env.sky_light[i] - env.ground_light[i]) * hemi)
                    * skyl
                    * sky_facing;
                *c *= (ambient + env.lamp[i] * lamp + env.floor) * ao
                    + env.sun[i] * lit * (0.55 + 0.45 * ao);
            }

            if s.ink > 0.0 {
                let width = (size * 0.9).max(0.085);
                let strength = s.ink * (0.085 / width * 1.6).min(1.0);
                let mut line = 0.0f32;
                for k in 0..4 {
                    if !side[k] && !above[k] {
                        line = line.max(1.0 - edge[k] / width);
                    }
                }
                if line > 0.0 {
                    let k = 1.0 - strength * (line * 1.5).min(1.0);
                    for c in &mut col {
                        *c *= k;
                    }
                }
            }
        }

        let fade = smoothstep(s.max_t * 0.7, s.max_t, h.t);
        let fog = (1.0 - (-env.fog * h.t).exp()).max(fade);
        for (c, haze) in col.iter_mut().zip(env.haze) {
            *c += (haze - *c) * fog;
        }
        col
    }
}

/// True if light and outlines should treat the voxel as filled: the world is
/// solid there, or something other than the viewer is stamped there.
fn opaque(frame: &Frame, cell: Cell) -> bool {
    if frame.world.room(cell.room).is_solid(cell.at) {
        return true;
    }
    match frame.overlay.and_then(|overlay| overlay.get(cell)) {
        Some(stamp) => Some(stamp.entity) != frame.viewer,
        None => false,
    }
}

/// The 3 x 3 block of voxels in the layer in front of a face: which are
/// filled, and how much flooded light the empty ones hold.
struct Ring {
    solid: [[bool; 3]; 3],
    lamp: [[u8; 3]; 3],
    sky: [[u8; 3]; 3],
    /// Light one voxel further out from the face. Flooded light has no
    /// direction of its own; whether it rises or falls going outward says
    /// whether the face looks toward its source.
    lamp_out: u8,
    sky_out: u8,
}

struct Corner {
    ao: f32,
    lamp: f32,
    sky: f32,
}

impl Ring {
    fn facing(near: u8, out: u8) -> f32 {
        match out.cmp(&near) {
            std::cmp::Ordering::Greater => 1.0,
            std::cmp::Ordering::Equal => 0.72,
            std::cmp::Ordering::Less => 0.46,
        }
    }

    /// Occlusion and light at one corner of the face, from the up-to-four
    /// empty voxels that meet there. `iu` and `iv` are 0 or 2.
    fn corner(&self, iu: usize, iv: usize) -> Corner {
        const LEVELS: [f32; 4] = [1.0, 0.74, 0.52, 0.34];
        let (s1, s2, sc) = (self.solid[iu][1], self.solid[1][iv], self.solid[iu][iv]);
        let occ = if s1 && s2 {
            3
        } else {
            s1 as usize + s2 as usize + sc as usize
        };

        let mut lamp = self.lamp[1][1] as f32;
        let mut sky = self.sky[1][1] as f32;
        let mut n = 1.0;
        let mut add = |iu: usize, iv: usize| {
            lamp += self.lamp[iu][iv] as f32;
            sky += self.sky[iu][iv] as f32;
            n += 1.0;
        };
        if !s1 {
            add(iu, 1);
        }
        if !s2 {
            add(1, iv);
        }
        if !sc && !(s1 && s2) {
            add(iu, iv);
        }
        let k = 1.0 / (n * REACH as f32);
        Corner {
            ao: LEVELS[occ],
            lamp: lamp * k,
            sky: sky * k,
        }
    }

    fn sample(frame: &Frame, h: &Hit, ua: usize, va: usize) -> Ring {
        let world = frame.world;
        let light = frame.light;
        let n = h.face.normal();
        // Outside the world there is nothing, lit by the sky.
        let mut ring = Ring {
            solid: [[false; 3]; 3],
            lamp: [[0; 3]; 3],
            sky: [[REACH; 3]; 3],
            lamp_out: 0,
            sky_out: REACH,
        };
        let set = |ring: &mut Ring, iu: usize, iv: usize, cell: Cell| {
            ring.solid[iu][iv] = opaque(frame, cell);
            ring.lamp[iu][iv] = light.lamp(cell);
            ring.sky[iu][iv] = light.sky(cell);
        };
        let offset = |iu: usize, iv: usize| {
            let mut d = IVec3::ZERO;
            d[ua] = iu as i32 - 1;
            d[va] = iv as i32 - 1;
            d
        };

        // The usual case: plain space in front of the face.
        let room = world.room(h.cell.room);
        let front = Cell {
            room: h.cell.room,
            at: h.cell.at + n,
        };
        let plain = h.via.is_none()
            && room.contains(front.at)
            && Face::ALL
                .into_iter()
                .all(|face| world.seam_at(front, face).is_none());
        if plain {
            for iu in 0..3 {
                for iv in 0..3 {
                    let at = front.at + offset(iu, iv);
                    if room.contains(at) {
                        let cell = Cell {
                            room: front.room,
                            at,
                        };
                        set(&mut ring, iu, iv, cell);
                    }
                }
            }
            let out = Cell {
                room: front.room,
                at: front.at + n,
            };
            if room.contains(out.at) && !opaque(frame, out) {
                ring.lamp_out = light.lamp(out);
                ring.sky_out = light.sky(out);
            } else {
                ring.lamp_out = ring.lamp[1][1];
                if room.contains(out.at) {
                    ring.sky_out = ring.sky[1][1];
                }
            }
            return ring;
        }

        // Next to a seam: ask the topology what is really there. The empty
        // voxel in front of the face is in the frame the ray came from.
        let turn = h
            .via
            .map_or(Orientation::IDENTITY, |id| world.seam(id).map.rot);
        let back = turn.inverse();
        for iu in 0..3 {
            for iv in 0..3 {
                if let Some((there, _)) = world.walk(h.from, back.apply(offset(iu, iv))) {
                    set(&mut ring, iu, iv, there);
                }
            }
        }
        ring.solid[1][1] = false;
        ring.lamp_out = ring.lamp[1][1];
        ring.sky_out = ring.sky[1][1];
        match world.walk(h.from, back.apply(n)) {
            Some((there, _)) => {
                if !opaque(frame, there) {
                    ring.lamp_out = light.lamp(there);
                    ring.sky_out = light.sky(there);
                }
            }
            None => ring.sky_out = REACH,
        }
        ring
    }
}

/// Is the sun hidden from the point `p` on the face that `h` hit?
fn in_shadow(frame: &Frame, h: &Hit, p: Vec3, sun_dir: [f32; 3]) -> bool {
    let n = h.face.normal();
    let lift = Vec3([n[0] as f32, n[1] as f32, n[2] as f32]) * 0.003;
    let mut origin = p + lift;
    let mut dir = Vec3(sun_dir);
    if let Some(id) = h.via {
        // The empty voxel in front of the face is in the frame the ray came
        // from: express the shadow ray there. In that frame the hit voxel
        // would be one step beyond `from`.
        let back = frame.world.seam(id).map.rot.inverse();
        let rel = back.apply_f(origin - h.cell.at.centre());
        let virtual_hit = h.from.at - back.apply(n);
        origin = rel + virtual_hit.centre();
        dir = back.apply_f(dir);
    }
    let ray = Ray {
        start: h.from,
        origin,
        dir,
    };
    // The viewer's own voxels block the sun like any others.
    let opts = TraceOpts {
        max_t: 140.0,
        max_steps: 700,
        skip: None,
    };
    trace(frame, ray, &opts).kind == HitKind::Solid
}

/// The colour of the sky in direction `d`, in the room's frame.
pub(crate) fn sky(env: &Env, d: Vec3) -> [f32; 3] {
    let y = d[1];
    let to = if y >= 0.0 { env.zenith } else { env.nadir };
    let k = y.abs().sqrt();
    std::array::from_fn(|i| env.horizon[i] + (to[i] - env.horizon[i]) * k)
}

/// Is the voxel across `face` of a solid voxel also filled? Follows a seam
/// if there is one, and treats the edge of a room as more of the same.
fn solid_across(frame: &Frame, cell: Cell, face: Face) -> bool {
    match frame.world.neighbour(cell, face) {
        Some((there, _)) => opaque(frame, there),
        None => true,
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash3(c: IVec3) -> u32 {
    let h = c[0]
        .wrapping_mul(374_761_393)
        .wrapping_add(c[1].wrapping_mul(668_265_263))
        .wrapping_add(c[2].wrapping_mul(1_440_662_683)) as u32;
    let h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

fn unit(h: u32) -> f32 {
    (h & 0xFFFF) as f32 / 65535.0
}

/// Which block of a material's pattern a voxel belongs to.
fn block_of(m: &Material, c: IVec3, face_axis: usize) -> IVec3 {
    let n = m.scale.max(1);
    let div = |a: i32, b: i32| a.div_euclid(b);
    match m.pattern {
        Pattern::Plain => c,
        Pattern::Tile | Pattern::Checker => IVec3([div(c[0], n), div(c[1], n), div(c[2], n)]),
        Pattern::Brick => {
            if face_axis == 1 {
                return IVec3([div(c[0], n), c[1], div(c[2], n)]);
            }
            let course = div(c[1], 2);
            let shift = if course.rem_euclid(2) == 0 { 0 } else { n / 2 };
            IVec3([div(c[0] + c[2] + shift, n), course, 0])
        }
        Pattern::Plank => IVec3([div(c[0], n), c[1], div(c[2], 2)]),
    }
}

fn albedo(m: &Material, c: IVec3, face_axis: usize, edge: [f32; 4], size: f32) -> [f32; 3] {
    let b = block_of(m, c, face_axis);
    let checker = m.pattern == Pattern::Checker;
    let col = if checker && (b[0] + b[1] + b[2]).rem_euclid(2) != 0 {
        m.color2
    } else {
        m.color
    };
    let mut k = 1.0 + m.grain * (2.0 * unit(hash3(b)) - 1.0);

    if m.pattern != Pattern::Plain && !checker {
        // Mortar: a dark line wherever the next voxel over is another block.
        let (ua, va) = ((face_axis + 1) % 3, (face_axis + 2) % 3);
        let width = (size * 0.8).max(0.07);
        let strength = 0.3 * (0.07 / width * 1.5).min(1.0);
        let steps = [(ua, -1), (ua, 1), (va, -1), (va, 1)];
        let mut line = 0.0f32;
        for (i, (axis, d)) in steps.into_iter().enumerate() {
            if edge[i] >= width {
                continue;
            }
            let mut next = c;
            next[axis] += d;
            if block_of(m, next, face_axis) != b {
                line = line.max(1.0 - edge[i] / width);
            }
        }
        k *= 1.0 - strength * line;
    }
    [col[0] * k, col[1] * k, col[2] * k]
}

const BAYER4: [f32; 16] = [
    0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0,
];

/// Linear colour to a pixel: a soft shoulder, display gamma, and dither.
pub(crate) fn pack(c: [f32; 3], x: u32, y: u32, levels: u32) -> u32 {
    let d = BAYER4[((y & 3) * 4 + (x & 3)) as usize] / 16.0;
    let mut out = 0xFF00_0000u32;
    for (i, &channel) in c.iter().enumerate() {
        let lin = channel.max(0.0);
        let v = (lin / (1.0 + 0.18 * lin) * 1.18).sqrt().clamp(0.0, 1.0);
        let b = if levels > 1 {
            let n = (levels - 1) as f32;
            (v * n + d).floor() / n * 255.0
        } else {
            v * 255.0 + d
        };
        out |= (b.clamp(0.0, 255.0) as u32) << (i * 8);
    }
    out
}
