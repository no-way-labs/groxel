//! The four worlds.
//!
//! Three kinds of test. The first holds the worlds to a recording of
//! themselves: the voxels, seams, light and starting places of a reference
//! build, kept as hashes, so that a change to a world is a change someone
//! made on purpose. The second is that each world closes the way it claims
//! to. The third covers the overlay that figures are drawn into.

use groxel::{
    Cell, EntityId, Face, HitKind, Orientation, Ray, RoomId, Scene, SeamKind, Skip, Sprite,
    TraceOpts, World, ivec3, trace, vec3,
};
use groxel_app::{Demo, Stamped, VIEWER, gallery, glows, mat, monk, palette, tower, well, you};
use groxel_body::{Body, Input};
use groxel_render::{Material, Pattern, Renderer, Settings};
use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::LazyLock;

/// Building the worlds floods six million voxels with light, so the tests
/// share one demo. A test that moves the walkers takes a copy.
static DEMO: LazyLock<Demo> = LazyLock::new(|| Demo::new().expect("the worlds fit together"));

const DT: f32 = 1.0 / 60.0;

const STILL: Input = Input {
    forward: 0.0,
    strafe: 0.0,
    rise: 0.0,
    turn: 0.0,
    look: 0.0,
    jump: false,
    run: false,
};

const FORWARD: Input = Input {
    forward: 1.0,
    ..STILL
};

fn room(name: &str) -> RoomId {
    let at = DEMO.world.rooms().iter().position(|r| r.name() == name);
    RoomId(at.expect("a room of that name") as u32)
}

/// Every voxel of a room, in index order: x fastest, then y, then z.
fn cells(world: &World, id: RoomId) -> impl Iterator<Item = Cell> {
    let d = world.room(id).dims();
    (0..d[2]).flat_map(move |z| {
        (0..d[1]).flat_map(move |y| {
            (0..d[0]).map(move |x| Cell {
                room: id,
                at: ivec3(x, y, z),
            })
        })
    })
}

/// FNV-1a, 64 bits: enough to say two long lists of numbers are the same
/// list.
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn int(&mut self, v: i64) {
        self.bytes(&v.to_le_bytes());
    }

    fn float(&mut self, v: f32) {
        self.bytes(&v.to_bits().to_le_bytes());
    }
}

// --- The worlds are the recorded ones ---------------------------------------
//
// The numbers below were recorded from the reference build of the worlds, an
// earlier implementation of this engine, by a program that walks every voxel
// of every room. They are hashes, so a failure says only that something
// differs; the other tests say what.
//
// Three numbers are not the reference build's: the sky light of the gallery,
// the well and the hall. The reference build let no sky light through a seam
// out of an outdoors room, so the gallery was not lit through its door to
// the tower, nor the well and the hall through their doors from the gallery.
// They are now, and those three hashes were recorded again from this engine.

/// What the reference build has in one room.
struct Reference {
    name: &'static str,
    dims: [i32; 3],
    outdoors: bool,
    /// Voxels that are not empty.
    solid: u64,
    /// The material of every voxel, in index order, one byte each.
    materials: u64,
    /// How many voxel faces are seams.
    seam_faces: u64,
    /// For every seam face, by voxel index and then by face: the voxel's
    /// index, the face's number, the room and voxel it leads to, where the
    /// seam's turn sends +x, +y and +z, and whether it stops bodies. Each
    /// number is hashed as eight little-endian bytes.
    seams: u64,
    /// The lamp light of every voxel, in index order, one byte each.
    lamp: u64,
    /// The sky light, likewise.
    sky: u64,
    /// The room's lighting: the bits of zenith, horizon, nadir, haze,
    /// sun_dir, sun, sky_light, ground_light, lamp, floor and fog, each
    /// number as four little-endian bytes.
    env: u64,
}

const REFERENCE: [Reference; 4] = [
    Reference {
        name: "gallery",
        dims: [46, 18, 32],
        outdoors: false,
        solid: 5404,
        materials: 0x5a9f_ac6e_03f8_4ded,
        seam_faces: 456,
        seams: 0xb164_c765_5075_4765,
        lamp: 0x582d_01b9_474e_bbeb,
        sky: 0x4422_a920_2b4a_e547,
        env: 0x7f71_2c27_6a59_39c8,
    },
    Reference {
        name: "well",
        dims: [42, 32, 42],
        outdoors: false,
        solid: 11909,
        materials: 0x0e22_ddf5_965f_ed15,
        seam_faces: 3560,
        seams: 0x2247_c972_b28e_e07e,
        lamp: 0xc50a_a6f6_1056_c95d,
        sky: 0x74d2_c4dc_9961_a8d7,
        env: 0x2ed4_4993_9198_3135,
    },
    Reference {
        name: "tower",
        dims: [120, 400, 120],
        outdoors: true,
        solid: 317_241,
        materials: 0x0ba5_0157_e6e0_c5ed,
        seam_faces: 31312,
        seams: 0x1c3c_df3e_1a66_d1ad,
        lamp: 0x63b4_a592_f5c1_5968,
        sky: 0x7eea_ebd3_4e84_f325,
        env: 0x5d72_630c_77f8_7425,
    },
    Reference {
        name: "hall",
        dims: [22, 22, 56],
        outdoors: false,
        solid: 4715,
        materials: 0xb6e6_38bc_cf1f_1b2e,
        seam_faces: 1000,
        seams: 0xc022_da7d_2a48_1c18,
        lamp: 0x477a_7a49_04c9_2c66,
        sky: 0x14a8_3956_a762_e83c,
        env: 0x515f_02be_f3b9_b26a,
    },
];

#[test]
fn the_worlds_fit_together() {
    let demo = Demo::new();
    assert!(demo.is_ok(), "{:?}", demo.err());
    let demo = demo.unwrap();
    let names: Vec<&str> = demo.world.rooms().iter().map(|r| r.name()).collect();
    assert_eq!(names, ["gallery", "well", "tower", "hall"]);
    // The mirror; three doors, the well's wrap, the tower's cut and the
    // hall's turn, each a seam there and a seam back.
    assert_eq!(demo.world.seams().len(), 1 + 2 * 6);
    let sight_only = |s: &&groxel::Seam| s.kind == SeamKind::SightOnly;
    assert_eq!(demo.world.seams().iter().filter(sight_only).count(), 1);
}

#[test]
fn every_voxel_is_the_recorded_one() {
    let world = &DEMO.world;
    assert_eq!(world.rooms().len(), REFERENCE.len());
    for (i, want) in REFERENCE.iter().enumerate() {
        let id = RoomId(i as u32);
        let r = world.room(id);
        assert_eq!(r.name(), want.name);
        assert_eq!(r.dims().0, want.dims, "{}", want.name);
        assert_eq!(DEMO.is_outdoors(id), want.outdoors, "{}", want.name);

        let (mut solid, mut materials) = (0, Fnv::new());
        for cell in cells(world, id) {
            let m = r.material(cell.at);
            solid += (m != 0) as u64;
            materials.bytes(&[m]);
        }
        assert_eq!(solid, want.solid, "solid voxels in the {}", want.name);
        assert_eq!(materials.0, want.materials, "voxels of the {}", want.name);
    }
}

#[test]
fn every_seam_face_leads_where_it_was_recorded_to() {
    let world = &DEMO.world;
    for (i, want) in REFERENCE.iter().enumerate() {
        let id = RoomId(i as u32);
        let (mut faces, mut hash) = (0, Fnv::new());
        for (index, cell) in cells(world, id).enumerate() {
            for (number, face) in Face::ALL.into_iter().enumerate() {
                let Some(seam) = world.seam_at(cell, face) else {
                    continue;
                };
                let (to, via) = world.neighbour(cell, face).expect("a seam leads on");
                assert_eq!(via, Some(seam));
                let seam = world.seam(seam);
                faces += 1;
                hash.int(index as i64);
                hash.int(number as i64);
                hash.int(to.room.0 as i64);
                for v in to.at.0 {
                    hash.int(v as i64);
                }
                for axis in [ivec3(1, 0, 0), ivec3(0, 1, 0), ivec3(0, 0, 1)] {
                    for v in seam.map.rot.apply(axis).0 {
                        hash.int(v as i64);
                    }
                }
                hash.int((seam.kind == SeamKind::SightOnly) as i64);
            }
        }
        assert_eq!(faces, want.seam_faces, "seam faces in the {}", want.name);
        assert_eq!(hash.0, want.seams, "seams of the {}", want.name);
    }
}

#[test]
fn every_voxel_is_lit_as_recorded() {
    for (i, want) in REFERENCE.iter().enumerate() {
        let id = RoomId(i as u32);
        let (mut lamp, mut sky) = (Fnv::new(), Fnv::new());
        for cell in cells(&DEMO.world, id) {
            lamp.bytes(&[DEMO.light.lamp(cell)]);
            sky.bytes(&[DEMO.light.sky(cell)]);
        }
        assert_eq!(lamp.0, want.lamp, "lamp light in the {}", want.name);
        assert_eq!(sky.0, want.sky, "sky light in the {}", want.name);

        let e = DEMO.env(id);
        let mut env = Fnv::new();
        let colours = [
            e.zenith,
            e.horizon,
            e.nadir,
            e.haze,
            e.sun_dir,
            e.sun,
            e.sky_light,
            e.ground_light,
            e.lamp,
        ];
        for v in colours.into_iter().flatten().chain([e.floor, e.fog]) {
            env.float(v);
        }
        assert_eq!(env.0, want.env, "lighting of the {}", want.name);
    }
}

#[test]
fn the_palette_and_the_figures_are_the_recorded_ones() {
    // Every material: colour, second colour, glow, pattern (as its number in
    // the order plain, brick, tile, checker, plank), scale and grain.
    let mut hash = Fnv::new();
    for m in palette() {
        for v in m.color.into_iter().chain(m.color2).chain([m.glow]) {
            hash.float(v);
        }
        let pattern = match m.pattern {
            Pattern::Plain => 0,
            Pattern::Brick => 1,
            Pattern::Tile => 2,
            Pattern::Checker => 3,
            Pattern::Plank => 4,
        };
        hash.bytes(&[pattern]);
        hash.bytes(&m.scale.to_le_bytes());
        hash.float(m.grain);
    }
    assert_eq!(hash.0, 0x401f_a938_9087_8182);

    // A sprite: its size, its origin, then its voxels in index order.
    let print = |sprite: &Sprite| {
        let mut hash = Fnv::new();
        let (d, o) = (sprite.dims(), sprite.origin());
        for v in d.0.into_iter().chain(o.0) {
            hash.bytes(&v.to_le_bytes());
        }
        for z in 0..d[2] {
            for y in 0..d[1] {
                for x in 0..d[0] {
                    hash.bytes(&[sprite.material(ivec3(x, y, z))]);
                }
            }
        }
        hash.0
    };
    assert_eq!(print(&monk()[0]), 0x098b_1909_b54b_a957);
    assert_eq!(print(&monk()[1]), 0x8dee_fb1d_38bd_c087);
    assert_eq!(print(&you()[0]), 0xa37d_252c_9525_3780);
    assert_eq!(print(&you()[1]), 0x81c7_a63b_8a05_d838);
}

#[test]
fn spots_and_walkers_start_where_they_were_recorded() {
    // A spot: its name, room, position, heading, pitch and whether it flies.
    let mut hash = Fnv::new();
    for s in &DEMO.spots {
        hash.bytes(s.name.as_bytes());
        hash.bytes(&s.room.0.to_le_bytes());
        for v in s.pos.0.into_iter().chain([s.yaw, s.pitch]) {
            hash.float(v);
        }
        hash.bytes(&[s.fly as u8]);
        assert_eq!(s.frame, Orientation::IDENTITY, "{}", s.name);
    }
    assert_eq!(hash.0, 0x84db_8c64_41a6_da77);

    // A walker: its room, its position, and the corner it is heading for.
    // Headings are left out: they come from an arctangent, which need not
    // round the same way on every machine.
    let mut hash = Fnv::new();
    for k in &DEMO.walkers {
        hash.bytes(&k.body.room.0.to_le_bytes());
        for v in k.body.pos.0 {
            hash.float(v);
        }
        hash.bytes(&(k.next as u32).to_le_bytes());
    }
    assert_eq!(hash.0, 0xa179_accc_f705_89b4);

    let in_room = |name: &str| {
        let id = room(name);
        DEMO.walkers.iter().filter(|k| k.body.room == id).count()
    };
    assert_eq!(
        [in_room("well"), in_room("tower"), in_room("hall")],
        [7, 16, 4]
    );
}

// --- The palette -----------------------------------------------------------

#[test]
fn lamps_markers_and_windows_glow() {
    let glowing: Vec<u8> = (0..=255).filter(|&m| glows(m)).collect();
    assert_eq!(glowing, [mat::LAMP, mat::MARKER, mat::WINDOW]);
}

#[test]
fn colours_are_kept_in_linear_light() {
    // Written as 0.74, 0.13, 0.10; stored squared.
    let lacquer = palette()[mat::LACQUER as usize];
    assert_eq!(lacquer.color, [0.74 * 0.74, 0.13 * 0.13, 0.10 * 0.10]);
    assert_eq!(lacquer.grain, 0.04);
    let checker = palette()[mat::CHECKER as usize];
    assert_eq!(checker.pattern, Pattern::Checker);
    assert_eq!(checker.color2, [0.20 * 0.20, 0.20 * 0.20, 0.23 * 0.23]);
    // A material no world uses is the renderer's default, squared too.
    let plain = Material::default();
    let unused = palette()[200];
    assert_eq!(unused.color, plain.color.map(|c| c * c));
    assert_eq!(unused.color2, plain.color2.map(|c| c * c));
}

#[test]
fn painting_gives_a_renderer_the_palette_and_each_room_s_lighting() {
    let mut renderer = Renderer::new(Settings::default());
    DEMO.paint(&mut renderer);
    for id in 0..=255 {
        assert_eq!(*renderer.material(id), palette()[id as usize], "{id}");
    }
    for want in &REFERENCE {
        let id = room(want.name);
        assert_eq!(*renderer.env(id), DEMO.env(id), "{}", want.name);
    }
    // No two rooms are lit alike, and only the tower has a sun.
    let suns: Vec<bool> = REFERENCE
        .iter()
        .map(|want| DEMO.env(room(want.name)).sun != [0.0; 3])
        .collect();
    assert_eq!(suns, [false, false, true, false]);
    assert_ne!(DEMO.env(room("gallery")), DEMO.env(room("hall")));
}

// --- Spots -----------------------------------------------------------------

#[test]
fn each_spot_is_inside_its_room_in_an_empty_voxel() {
    let names: Vec<&str> = DEMO.spots.iter().map(|s| s.name).collect();
    assert_eq!(names, ["gallery", "well", "tower", "overlook", "hall"]);
    for s in &DEMO.spots {
        assert_eq!(DEMO.spot(s.name), Some(*s));
        let r = DEMO.world.room(s.room);
        let at = s.pos.floor();
        assert!(
            r.contains(at),
            "{}: {:?} is outside the room",
            s.name,
            s.pos
        );
        assert!(!r.is_solid(at), "{}: {:?} is solid", s.name, s.pos);
        // The middle of a body put there is in the room too.
        let body = Demo::body_at(s);
        assert!(r.contains(body.anchor().at), "{}", s.name);
        assert!(!r.is_solid(body.anchor().at), "{}", s.name);
        assert_eq!((body.yaw, body.pitch, body.fly), (s.yaw, s.pitch, s.fly));
    }
    let expected_rooms = ["gallery", "well", "tower", "tower", "hall"];
    for (s, name) in DEMO.spots.iter().zip(expected_rooms) {
        assert_eq!(DEMO.world.room(s.room).name(), name);
    }
    assert_eq!(DEMO.spot("nowhere"), None);
    assert!(DEMO.spot("overlook").unwrap().fly);
}

// --- Walkers ---------------------------------------------------------------

#[test]
fn after_ten_seconds_every_walker_is_on_the_ground_in_its_room() {
    let mut d = DEMO.clone();
    let rooms: Vec<RoomId> = d.walkers.iter().map(|k| k.body.room).collect();
    for _ in 0..600 {
        d.tick(DT);
    }
    for (n, (k, home)) in d.walkers.iter().zip(&rooms).enumerate() {
        let b = &k.body;
        assert_eq!(b.room, *home, "walker {n} left its room");
        assert!(b.on_ground, "walker {n} is in the air at {:?}", b.pos);
        let r = d.world.room(b.room);
        let middle = b.anchor().at;
        assert!(r.contains(middle), "walker {n} is outside at {:?}", b.pos);
        assert!(
            !r.is_solid(middle),
            "walker {n} is in a wall at {:?}",
            b.pos
        );
        // 4.2 to 4.6 voxels a second for ten seconds.
        assert!(
            (k.odometer - b.walk_speed * 10.0).abs() < 0.01,
            "walker {n}"
        );
    }
    // They go round the well, round the tower and down the hall, so in ten
    // seconds some have been through the wrap, the cut and the turn.
    for name in ["well", "tower", "hall"] {
        let id = room(name);
        let here = d.walkers.iter().filter(|k| k.body.room == id);
        let crossings: u32 = here.map(|k| k.body.crossings).sum();
        assert!(crossings >= 1, "no walker has crossed a seam in the {name}");
    }
    // In the hall a crossing is a quarter turn: those walkers stand on the
    // next wall round.
    for (k, was) in d.walkers.iter().zip(&DEMO.walkers) {
        if k.body.room == room("hall") && k.body.crossings == 1 {
            let turned = was.body.frame.then(Orientation::quarter_turn(2, 1));
            assert_eq!(k.body.frame, turned);
        } else if k.body.room != room("hall") {
            assert_eq!(k.body.frame, Orientation::IDENTITY);
        }
    }
}

#[test]
fn after_ten_seconds_the_walkers_are_where_they_were_recorded() {
    let mut d = DEMO.clone();
    for _ in 0..600 {
        d.tick(DT);
    }
    // The reference build's walkers after the same 600 ticks: the room, the
    // corner being walked to, the seams crossed so far, and the feet. On the
    // machine this was written on the two builds' positions are the same to
    // the last bit. A fiftieth of a voxel allows for sines and arctangents
    // that round differently elsewhere.
    #[rustfmt::skip]
    let recorded: [(u32, usize, u32, [f32; 3]); 27] = [
        (1, 2, 0, [35.596, 15.000, 22.451]),
        (1, 3, 0, [34.556, 20.000, 6.663]),
        (1, 3, 0, [18.745, 25.000, 6.401]),
        (1, 0, 1, [6.465, -3.000, 11.569]),
        (1, 0, 1, [6.400, 2.000, 27.945]),
        (1, 1, 1, [15.418, 6.000, 35.582]),
        (1, 1, 0, [32.078, 11.000, 35.600]),
        (2, 2, 0, [41.021, 207.000, 70.654]),
        (2, 2, 0, [41.000, 199.000, 53.714]),
        (2, 3, 0, [45.512, 194.000, 41.080]),
        (2, 3, 0, [62.069, 186.000, 41.000]),
        (2, 0, 0, [78.667, 179.000, 41.305]),
        (2, 0, 0, [78.999, 173.000, 57.852]),
        (2, 0, 1, [79.000, 225.000, 74.720]),
        (2, 1, 1, [66.437, 220.000, 78.995]),
        (2, 1, 0, [49.497, 212.000, 79.000]),
        (2, 2, 1, [64.354, 185.000, 44.610]),
        (2, 2, 0, [46.364, 194.000, 44.600]),
        (2, 3, 0, [44.602, 202.000, 60.549]),
        (2, 0, 0, [47.181, 211.000, 75.248]),
        (2, 0, 0, [64.958, 219.000, 75.400]),
        (2, 1, 0, [75.371, 228.000, 67.851]),
        (2, 1, 1, [75.400, 177.000, 50.632]),
        (3, 0, 0, [6.000, 1.000, 52.000]),
        (3, 0, 1, [16.000, 21.000, 8.000]),
        (3, 0, 1, [1.000, 16.000, 20.000]),
        (3, 0, 1, [6.000, 1.000, 32.000]),
    ];
    assert_eq!(d.walkers.len(), recorded.len());
    for (n, (k, (room, next, crossings, pos))) in d.walkers.iter().zip(recorded).enumerate() {
        let b = &k.body;
        let ctx = format!("walker {n}: {:?} against {pos:?}", b.pos);
        assert_eq!(
            (b.room.0, k.next, b.crossings),
            (room, next, crossings),
            "{ctx}"
        );
        assert!((b.pos - groxel::Vec3(pos)).length() < 0.02, "{ctx}");
    }
}

#[test]
fn walkers_keep_walking_and_never_fall_out_of_the_world() {
    let mut d = DEMO.clone();
    for _ in 0..60 * 60 {
        d.tick(DT);
    }
    for (n, k) in d.walkers.iter().enumerate() {
        let b = &k.body;
        let ctx = format!(
            "walker {n} in the {}: pos {:?} next {} crossings {} ground {}",
            d.world.room(b.room).name(),
            b.pos,
            k.next,
            b.crossings,
            b.on_ground
        );
        assert!(b.crossings >= 1, "{ctx}");
        assert!(b.on_ground, "{ctx}");
    }
}

// --- Each world closes -----------------------------------------------------

#[test]
fn the_well_returns_you_to_the_step_you_left() {
    let d = &*DEMO;
    let mut b = Demo::body_at(&d.spot("well").unwrap());
    b.pos = vec3(4.0, (well::TOP + 1) as f32, 4.0);
    let start = b.pos;
    // Walk the descending lane once round.
    let lane = well::DOWN_LANE;
    let (mut corner, mut steps, mut laps) = (1, 0, 0);
    while laps < 1 && steps < 60 * 120 {
        let target = lane[corner];
        let dx = target[0] - b.pos[0];
        let dz = target[1] - b.pos[2];
        if dx * dx + dz * dz < 0.3 * 0.3 {
            if corner == 0 {
                laps += 1;
            }
            corner = (corner + 1) % lane.len();
        }
        b.yaw = b.yaw_toward(vec3(target[0], 0.0, target[1]));
        b.update(&d.world, &FORWARD, DT);
        steps += 1;
    }
    assert_eq!(laps, 1);
    assert_eq!(b.crossings, 1);
    assert!((start[0] - b.pos[0]).abs() < 0.4, "{:?}", b.pos);
    assert!((start[1] - b.pos[1]).abs() < 1e-3, "{:?}", b.pos);
    assert!((start[2] - b.pos[2]).abs() < 0.4, "{:?}", b.pos);
}

#[test]
fn once_round_the_tower_always_downhill_and_you_are_back_at_the_gold_door() {
    let d = &*DEMO;
    let mut b = Demo::body_at(&d.spot("tower").unwrap());
    for _ in 0..60 {
        b.update(&d.world, &STILL, DT);
    }
    let start = b.pos;

    let mut highest = b.pos[1];
    let mut climbed = false;
    // The lane between the two files of walkers: a square about the middle
    // of the room, whose far side is at 120 - 77.2.
    let (m, far) = (tower::MID, 42.8);
    let lane = [[m, m], [far, m], [far, far], [m, far], [start[0], start[2]]];
    for corner in lane {
        let mut t = 0.0;
        while t < 60.0 {
            let dx = corner[0] - b.pos[0];
            let dz = corner[1] - b.pos[2];
            if dx * dx + dz * dz < 0.25 * 0.25 {
                break;
            }
            let crossings = b.crossings;
            let y = b.pos[1];
            b.yaw = b.yaw_toward(vec3(corner[0], 0.0, corner[1]));
            b.update(&d.world, &FORWARD, DT);
            // Within one frame of reference, the walker never goes up.
            if b.crossings == crossings && b.pos[1] > y + 1e-3 {
                climbed = true;
            }
            highest = highest.max(b.pos[1]);
            t += DT;
        }
    }
    assert!(!climbed);
    assert_eq!(b.crossings, 1);
    assert!((start[0] - b.pos[0]).abs() < 0.3, "{:?}", b.pos);
    assert!((start[2] - b.pos[2]).abs() < 0.3, "{:?}", b.pos);
    // Down sixty voxels of stairs, and at the height it started from.
    assert!((start[1] - b.pos[1]).abs() < 1e-3, "{:?}", b.pos);
    // The cut lifts it by the whole drop at once, from the lowest step.
    let top = start[1] + (tower::DROP - 7) as f32;
    assert!((top - highest).abs() < 1e-3, "{highest}");
}

#[test]
fn four_lengths_of_the_hall_stand_you_on_every_wall_and_bring_you_home() {
    let d = &*DEMO;
    let mut b = Demo::body_at(&d.spot("hall").unwrap());
    for _ in 0..60 {
        b.update(&d.world, &STILL, DT);
    }
    let start = b.pos;
    let mut ups = [Face::PosX; 4];
    while b.crossings < 4 {
        ups[b.crossings as usize] = b.up_face();
        b.update(&d.world, &FORWARD, DT);
        assert!(b.on_ground || b.vel_up > -3.0);
    }
    assert_eq!(ups, [Face::PosY, Face::NegX, Face::NegY, Face::PosX]);
    assert_eq!(b.frame, Orientation::IDENTITY);
    while b.pos[2] < start[2] {
        b.update(&d.world, &FORWARD, DT);
    }
    assert!((start[0] - b.pos[0]).abs() < 1e-2, "{:?}", b.pos);
    assert!((start[1] - b.pos[1]).abs() < 1e-2, "{:?}", b.pos);
}

#[test]
fn each_gallery_door_leads_to_its_world_and_back() {
    let d = &*DEMO;
    let home = d.spot("gallery").unwrap().room;
    for (c, name) in gallery::CENTRES.into_iter().zip(["well", "tower", "hall"]) {
        let mut b = Body::new(home, vec3(c as f32, 1.0, 24.0));
        for _ in 0..60 * 2 {
            b.update(&d.world, &FORWARD, DT);
        }
        assert_eq!(d.world.room(b.room).name(), name);
        assert!(b.on_ground, "{name}");
        // Turn round and walk back.
        b.yaw += PI;
        let mut i = 0;
        while i < 60 * 3 && b.room != home {
            b.update(&d.world, &FORWARD, DT);
            i += 1;
        }
        assert_eq!(b.room, home, "{name}");
        assert_eq!(b.frame, Orientation::IDENTITY, "{name}");
    }
}

#[test]
fn the_gallery_mirror_shows_the_viewer() {
    let d = &*DEMO;
    let mut b = Demo::body_at(&d.spot("gallery").unwrap());
    b.pos = vec3(23.5, 1.0, 7.5);
    b.yaw = PI; // face the mirror
    let stamped = d.stamp(Some(&b), 0.0, true);
    assert_eq!(stamped.viewer, Some(VIEWER));
    let v = b.view(&d.world);
    let scene = Scene {
        world: &d.world,
        overlay: &stamped.overlay,
    };
    let ray = Ray {
        start: v.cell,
        origin: v.pos,
        dir: v.fwd,
    };
    // The renderer's rule: the viewer's own voxels do not count until the
    // ray has crossed a seam and gone six voxels.
    let opts = TraceOpts {
        skip: Some(Skip {
            entity: VIEWER,
            near: 6.0,
        }),
        ..Default::default()
    };
    // Straight ahead at eye height: your own hood. 6.5 voxels to the glass
    // at z = 1, then 5 back to the front of the figure standing where you
    // are.
    let h = trace(&scene, ray, &opts);
    assert_eq!(h.kind, HitKind::Solid);
    assert_eq!(h.crossings, 1);
    assert_eq!(h.entity, Some(VIEWER));
    assert!(h.material == mat::SKIN || h.material == mat::YOU_DARK);
    assert!((h.t - 11.5).abs() < 1e-3, "{}", h.t);
    // Reflected, so the picture is the other way round.
    assert!(h.frame.is_reflection());

    // Without that rule the eye is inside its own head.
    let blocked = trace(&scene, ray, &TraceOpts::default());
    assert_eq!(blocked.entity, Some(VIEWER));
    assert_eq!(blocked.crossings, 0);
    // And with no figure the ray goes on through the place where it stood.
    let empty = trace(&d.world, ray, &TraceOpts::default());
    assert_eq!(empty.entity, None);
    assert!(empty.t > 20.0, "{}", empty.t);
}

// --- Figures ---------------------------------------------------------------

#[test]
fn a_sprite_is_read_from_text_bottom_layer_first() {
    // Two layers of one row each: the lower is "a.a", the upper ".b.".
    let s = Sprite::from_layers(&[&["a.a"], &[".b."]], &[('a', 4), ('b', 9)]);
    assert_eq!(s.dims(), ivec3(3, 2, 1));
    let voxels: Vec<u8> = (0..2)
        .flat_map(|y| (0..3).map(move |x| ivec3(x, y, 0)))
        .map(|c| s.material(c))
        .collect();
    assert_eq!(voxels, [4, 0, 4, 0, 9, 0]);

    // The monk is three wide, seven tall and three deep, and stands on the
    // middle of its bottom layer. Its one voxel of skin is in the front row,
    // five layers up.
    let m = &monk()[0];
    assert_eq!((m.dims(), m.origin()), (ivec3(3, 7, 3), ivec3(1, 0, 1)));
    assert_eq!(m.material(ivec3(1, 5, 2)), mat::SKIN);
    assert_eq!(m.material(ivec3(1, 5, 0)), mat::COWL);
    // You are the same shape in other colours.
    let y = &you()[0];
    assert_eq!(y.material(ivec3(1, 5, 2)), mat::SKIN);
    assert_eq!(y.material(ivec3(1, 0, 1)), mat::YOU);
    assert_eq!(y.material(ivec3(1, 6, 1)), mat::YOU_DARK);
    // Mid stride, two corners of the bottom layer are lifted.
    let stride = &monk()[1];
    assert_eq!(stride.material(ivec3(2, 0, 0)), 0);
    assert_eq!(stride.material(ivec3(0, 0, 2)), 0);
    assert_eq!(m.material(ivec3(2, 0, 0)), mat::ROBE);
}

/// Every overlay voxel that belongs to one entity.
fn voxels_of(d: &Demo, stamped: &Stamped, entity: EntityId) -> Vec<(Cell, u8)> {
    let ids = (0..d.world.rooms().len() as u32).map(RoomId);
    ids.flat_map(|id| cells(&d.world, id))
        .filter_map(|cell| {
            let stamp = stamped.overlay.get(cell)?;
            (stamp.entity == entity).then_some((cell, stamp.material))
        })
        .collect()
}

#[test]
fn stamping_leaves_the_world_exactly_as_it_was() {
    let d = &*DEMO;
    let print = |world: &World| {
        let mut hash = Fnv::new();
        for id in (0..world.rooms().len() as u32).map(RoomId) {
            for cell in cells(world, id) {
                hash.bytes(&[world.room(id).material(cell.at)]);
            }
        }
        hash.0
    };
    let before = print(&d.world);
    let b = Demo::body_at(&d.spot("well").unwrap());
    let mut stamped = d.stamp(Some(&b), 0.0, true);
    // Twenty-seven monks and you, about fifty voxels each.
    assert!(stamped.overlay.len() > 27 * 40, "{}", stamped.overlay.len());
    assert_eq!(print(&d.world), before);

    // Figures are drawn where the world is empty, never over it.
    for id in (0..d.world.rooms().len() as u32).map(RoomId) {
        for cell in cells(&d.world, id) {
            if stamped.overlay.get(cell).is_some() {
                assert!(!d.world.room(id).is_solid(cell.at), "{cell:?}");
            }
        }
    }
    stamped.overlay.clear();
    assert!(stamped.overlay.is_empty());
}

#[test]
fn a_frame_borrows_the_world_the_light_and_the_figures() {
    let d = &*DEMO;
    let b = Demo::body_at(&d.spot("hall").unwrap());
    let stamped = d.stamp(Some(&b), 0.0, true);
    let frame = d.frame(&stamped);
    assert!(std::ptr::eq(frame.world, &d.world));
    assert!(std::ptr::eq(frame.light, &d.light));
    assert!(std::ptr::eq(frame.overlay.unwrap(), &stamped.overlay));
    assert_eq!(frame.viewer, Some(VIEWER));
}

#[test]
fn every_walker_is_drawn_whole_and_the_viewer_only_when_shown() {
    let d = &*DEMO;
    // With nobody looking, every walker is drawn. All stand still at the
    // start, in the first of the two poses, which has 51 voxels.
    let all = d.stamp(None, 0.0, true);
    assert_eq!(all.viewer, None);
    assert!(voxels_of(d, &all, VIEWER).is_empty());
    let mut drawn = 0;
    for (n, k) in d.walkers.iter().enumerate() {
        let figure = voxels_of(d, &all, EntityId(n as u32 + 1));
        drawn += figure.len();
        if k.body.room == room("tower") {
            // The file that goes down by the parapet brushes it on two
            // sides of the tower, and a figure is not drawn into stone.
            assert!((36..=51).contains(&figure.len()), "walker {n}");
            continue;
        }
        assert_eq!(figure.len(), 51, "walker {n}");
        let of = |m: u8| figure.iter().filter(|(_, got)| *got == m).count();
        assert_eq!((of(mat::ROBE), of(mat::COWL), of(mat::SKIN)), (37, 13, 1));
    }
    assert_eq!(all.overlay.len(), drawn);

    // A viewer in the gallery is far from every walker.
    let b = Demo::body_at(&d.spot("gallery").unwrap());
    let shown = d.stamp(Some(&b), 0.0, true);
    assert_eq!(shown.viewer, Some(VIEWER));
    let me = voxels_of(d, &shown, VIEWER);
    assert_eq!(me.len(), 51);
    let of = |m: u8| me.iter().filter(|(_, got)| *got == m).count();
    assert_eq!(
        (of(mat::YOU), of(mat::YOU_DARK), of(mat::SKIN)),
        (37, 13, 1)
    );
    assert_eq!(shown.overlay.len(), all.overlay.len() + 51);

    let hidden = d.stamp(Some(&b), 0.0, false);
    assert_eq!(hidden.viewer, None);
    assert!(voxels_of(d, &hidden, VIEWER).is_empty());
    assert_eq!(hidden.overlay.len(), all.overlay.len());
}

#[test]
fn a_walker_within_arms_reach_of_the_viewer_is_left_out() {
    let d = &*DEMO;
    let entity = |n: usize| EntityId(n as u32 + 1);
    let near = d.walkers[3].body;
    let everyone = d.stamp(None, 0.0, false).overlay.len();
    // Standing four voxels from walker 3: it is left out, the rest are not.
    let mut b = Body::new(near.room, near.pos + vec3(0.0, 0.0, 4.0));
    let stamped = d.stamp(Some(&b), 0.0, false);
    assert!(voxels_of(d, &stamped, entity(3)).is_empty());
    assert_eq!(voxels_of(d, &stamped, entity(2)).len(), 51);
    assert_eq!(stamped.overlay.len(), everyone - 51);

    // Five voxels away is out of reach.
    b.pos = near.pos + vec3(0.0, 0.0, 5.0);
    let stamped = d.stamp(Some(&b), 0.0, false);
    assert_eq!(voxels_of(d, &stamped, entity(3)).len(), 51);

    // So is a viewer with the numbers of the first place, in another room.
    let elsewhere = Body::new(room("gallery"), near.pos + vec3(0.0, 0.0, 4.0));
    let stamped = d.stamp(Some(&elsewhere), 0.0, false);
    assert_eq!(voxels_of(d, &stamped, entity(3)).len(), 51);
}

#[test]
fn a_figure_strides_as_it_walks() {
    let mut d = DEMO.clone();
    // The second pose has two voxels fewer in its bottom layer.
    let count = |d: &Demo| voxels_of(d, &d.stamp(None, 0.0, false), EntityId(1)).len();
    for (odometer, voxels) in [
        (0.0, 51),
        (2.4, 51),
        (2.5, 49),
        (4.9, 49),
        (5.0, 51),
        (7.6, 49),
    ] {
        d.walkers[0].odometer = odometer;
        assert_eq!(count(&d), voxels, "after {odometer} voxels");
    }
    // The viewer strides by its own odometer.
    let b = Demo::body_at(&d.spot("gallery").unwrap());
    let me = |walked: f32| voxels_of(&d, &d.stamp(Some(&b), walked, true), VIEWER).len();
    assert_eq!([me(0.0), me(3.0), me(5.5)], [51, 49, 51]);
}

/// The one voxel of skin in a figure, which is its face.
fn face_of(d: &Demo, body: &Body) -> Cell {
    let mut overlay = groxel::Overlay::new(&d.world);
    groxel_app::put_body(&mut overlay, &d.world, body, &monk()[0], EntityId(9));
    let stamped = Stamped {
        overlay,
        viewer: None,
    };
    let skin: Vec<Cell> = voxels_of(d, &stamped, EntityId(9))
        .into_iter()
        .filter(|(_, m)| *m == mat::SKIN)
        .map(|(cell, _)| cell)
        .collect();
    assert_eq!(skin.len(), 1, "a figure has one face");
    skin[0]
}

#[test]
fn a_figure_faces_the_nearest_quarter_turn_to_its_heading() {
    let d = &*DEMO;
    let home = room("gallery");
    let mut b = Body::new(home, vec3(23.5, 1.0, 7.5));
    // The feet are in voxel (23, 1, 7). The face is five voxels up and one
    // voxel forward.
    for (yaw, forward) in [
        (0.0, [0, 1]),
        (FRAC_PI_2, [1, 0]),
        (PI, [0, -1]),
        (-FRAC_PI_2, [-1, 0]),
        (3.0 * FRAC_PI_2, [-1, 0]),
        // Nearest quarter: 40 degrees is still ahead, 50 is to the side.
        (0.70, [0, 1]),
        (0.87, [1, 0]),
        (-0.70, [0, 1]),
        (-0.87, [-1, 0]),
    ] {
        b.yaw = yaw;
        let want = Cell {
            room: home,
            at: ivec3(23 + forward[0], 6, 7 + forward[1]),
        };
        assert_eq!(face_of(d, &b), want, "heading {yaw}");
    }
}

#[test]
fn a_figure_stands_on_whatever_its_body_calls_the_floor() {
    let d = &*DEMO;
    let hall = room("hall");
    // The four hall walkers stand on the floor, the +x wall, the ceiling and
    // the -x wall, all facing down the hall. Each face is five voxels above
    // its own feet, whichever way that is.
    let faces: Vec<Cell> = d
        .walkers
        .iter()
        .filter(|k| k.body.room == hall)
        .map(|k| face_of(d, &k.body))
        .collect();
    let at = |x, y, z| Cell {
        room: hall,
        at: ivec3(x, y, z),
    };
    assert_eq!(
        faces,
        [
            at(6, 6, 9),    // feet on the floor at y = 1
            at(15, 6, 21),  // feet on the wall at x = 20
            at(16, 15, 33), // feet on the ceiling at y = 20
            at(6, 16, 45),  // feet on the wall at x = 1
        ]
    );
}

#[test]
fn a_figure_that_has_settled_stands_on_its_surface_not_in_it() {
    let mut d = DEMO.clone();
    // Long enough for every walker to drop the quarter voxel it starts above
    // its surface, and too short for any in the hall to reach its end.
    for _ in 0..30 {
        d.tick(DT);
    }
    let stamped = d.stamp(None, 0.0, false);
    let hall = room("hall");
    let mut seen = 0;
    for (n, k) in d.walkers.iter().enumerate() {
        if k.body.room != hall {
            continue;
        }
        seen += 1;
        assert!(k.body.on_ground, "walker {n}");
        // Whole: no layer of it is lost in the floor, wall or ceiling it
        // stands on.
        let figure = voxels_of(&d, &stamped, EntityId(n as u32 + 1));
        assert_eq!(figure.len(), 51, "walker {n}");
        // Its lowest layer is the voxel next to the surface, and the surface
        // is solid beneath that.
        let up = k.body.up_face();
        let along = |cell: &Cell| cell.at[up.axis()] * up.sign();
        let lowest = figure.iter().map(|(cell, _)| along(cell)).min().unwrap();
        let feet = figure
            .iter()
            .find(|(cell, _)| along(cell) == lowest)
            .unwrap()
            .0;
        let under = feet.at + up.opposite().normal();
        assert!(
            d.world.room(hall).is_solid(under),
            "walker {n} stands on air"
        );
    }
    assert_eq!(seen, 4);
}

#[test]
fn a_figure_on_a_wall_turns_about_its_own_up() {
    let d = &*DEMO;
    let hall = room("hall");
    // The walker on the hall's +x wall. Its up is the room's -x and its feet
    // are in voxel (20, 6, 20), so its face is five voxels out from the wall
    // and one voxel the way it is turned. A quarter turn of its heading is a
    // turn about its own up: from the room's +z to the room's +y, not to +x
    // as it would be for a figure on the floor.
    let on_wall = |k: &&groxel_app::Walker| k.body.room == hall && k.body.up_face() == Face::NegX;
    let walker = d
        .walkers
        .iter()
        .find(on_wall)
        .expect("a walker on that wall");
    let mut b = walker.body;
    for (yaw, forward) in [
        (0.0, ivec3(0, 0, 1)),
        (FRAC_PI_2, ivec3(0, 1, 0)),
        (PI, ivec3(0, 0, -1)),
        (-FRAC_PI_2, ivec3(0, -1, 0)),
    ] {
        b.yaw = yaw;
        let want = Cell {
            room: hall,
            at: ivec3(15, 6, 20) + forward,
        };
        assert_eq!(face_of(d, &b), want, "heading {yaw}");
    }
}

#[test]
fn a_figure_whose_feet_are_through_a_turned_seam_is_drawn_as_if_it_were_not() {
    // A shaft whose top opens onto its bottom a quarter turn round, or two,
    // or three. A body stands with its middle above the seam and its feet
    // below it, so its feet are found through the turn.
    for quarters in 1..4 {
        let mut w = World::new();
        let id = w.add_room("shaft", ivec3(9, 8, 9));
        let layer = |y: i32, face: Face| groxel::FaceRect {
            room: id,
            min: ivec3(0, y, 0),
            max: ivec3(8, y, 8),
            face,
        };
        let turn = Orientation::quarter_turn(1, quarters);
        w.glue(layer(7, Face::PosY), layer(0, Face::NegY), turn)
            .unwrap();

        for (yaw, forward) in [(0.0, ivec3(0, 0, 1)), (FRAC_PI_2, ivec3(1, 0, 0))] {
            let mut b = Body::new(id, vec3(2.5, -0.8, 6.5));
            b.yaw = yaw;
            assert_eq!(b.anchor().at, ivec3(2, 2, 6));
            let mut overlay = groxel::Overlay::new(&w);
            groxel_app::put_body(&mut overlay, &w, &b, &monk()[0], EntityId(1));
            let ctx = format!("{quarters} quarter turns, heading {yaw}");
            assert_eq!(overlay.len(), 51, "{ctx}");

            // Above the seam the figure is where it would be with no seam
            // there: its face five voxels above its feet, which are in the
            // voxel below (2, 0, 6), and one voxel the way it is turned.
            let get = |at| overlay.get(Cell { room: id, at });
            let face = ivec3(2, 4, 6) + forward;
            assert_eq!(get(face).map(|s| s.material), Some(mat::SKIN), "{ctx}");
            // The hood is the layer above, and nothing is above that.
            assert_eq!(get(ivec3(2, 5, 6)).map(|s| s.material), Some(mat::COWL));
            assert_eq!(get(ivec3(2, 6, 6)), None, "{ctx}");

            // Its lowest layer is through the seam, at the top of the shaft:
            // the nine voxels under it, each found by stepping down.
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let above = Cell {
                        room: id,
                        at: ivec3(2 + dx, 0, 6 + dz),
                    };
                    let (below, seam) = w.neighbour(above, Face::NegY).unwrap();
                    assert!(seam.is_some() && below.at[1] == 7, "{ctx}");
                    let stamp = overlay.get(below).map(|s| s.material);
                    assert_eq!(stamp, Some(mat::ROBE), "{ctx}: {below:?}");
                }
            }
        }
    }
}

#[test]
fn a_figure_in_a_doorway_is_in_both_rooms() {
    let d = &*DEMO;
    let (home, hall) = (room("gallery"), room("hall"));
    // In the gallery's door to the hall, facing through it. The last voxel
    // of the gallery is z = 31; the figure's front row is beyond it.
    let b = Body::new(home, vec3(35.5, 1.0, 31.5));
    let mut overlay = groxel::Overlay::new(&d.world);
    groxel_app::put_body(&mut overlay, &d.world, &b, &monk()[0], EntityId(9));
    let stamped = Stamped {
        overlay,
        viewer: None,
    };
    let figure = voxels_of(d, &stamped, EntityId(9));
    assert_eq!(figure.len(), 51);
    let here = figure.iter().filter(|(c, _)| c.room == home).count();
    let there = figure.iter().filter(|(c, _)| c.room == hall).count();
    // The back and middle rows of the figure are in the gallery, and its
    // front row is through the door.
    assert_eq!((here, there), (36, 15));

    // The door comes out of the hall's -x wall, a quarter turn round: the
    // gallery's +z is the hall's +x, and its +x is the hall's -z.
    let face = figure.iter().find(|(_, m)| *m == mat::SKIN).unwrap().0;
    assert_eq!(
        face,
        Cell {
            room: hall,
            at: ivec3(0, 6, 19)
        }
    );
    for (cell, _) in figure.iter().filter(|(c, _)| c.room == hall) {
        assert_eq!(cell.at[0], 0, "the front row is one voxel into the hall");
        assert!((18..=20).contains(&cell.at[2]), "{cell:?}");
    }
}

/// Step through the hall's door when it is underfoot and you arrive in the
/// gallery falling towards the wall with the mirror on it. You land on the
/// glass, stand on it, and can walk off it along the wall. (You once sank
/// into the mirror and stuck fast in the wall behind it.)
#[test]
fn falling_onto_the_gallerys_mirror_leaves_you_standing_on_it() {
    let demo = Demo::new().expect("the worlds build");
    let gallery = demo.spot("gallery").expect("a gallery").room;
    // Just inside the third door, with "up" towards the door wall: down is
    // towards the mirror, 30 voxels away.
    let mut me = groxel_body::Body::new(gallery, groxel::vec3(34.5, 5.0, 27.0));
    me.frame = groxel::Orientation::quarter_turn(0, 1);
    assert_eq!(me.up_face(), groxel::Face::PosZ);

    let still = groxel_body::Input::default();
    for _ in 0..240 {
        me.update(&demo.world, &still, 1.0 / 60.0);
    }
    assert!(me.on_ground);
    // Feet on the glass, which is the plane z = 1.
    assert!((me.pos[2] - 1.0).abs() < 1e-3, "{:?}", me.pos);

    // And not stuck: every way along the wall is open.
    for quarter in 0..4 {
        let mut walker = me;
        walker.yaw = quarter as f32 * std::f32::consts::FRAC_PI_2;
        let forward = groxel_body::Input {
            forward: 1.0,
            ..Default::default()
        };
        for _ in 0..30 {
            walker.update(&demo.world, &forward, 1.0 / 60.0);
        }
        let moved = (walker.pos - me.pos).length();
        assert!(moved > 2.0, "quarter {quarter}: moved {moved}");
        // Let a hop against whatever stopped it finish, then it must be
        // standing on the wall again, not in it.
        for _ in 0..60 {
            walker.update(&demo.world, &still, 1.0 / 60.0);
        }
        assert!(walker.on_ground, "quarter {quarter}");
        assert!(
            (walker.pos[2] - 1.0).abs() < 1e-3,
            "quarter {quarter}: {:?}",
            walker.pos
        );
    }
}

/// The same arrival with the demo's rule applied: the gallery has a floor,
/// so a body that comes in on its side is stood upright and lands on the
/// floor. The hall has none, and leaves a body as it is.
#[test]
fn the_gallery_stands_you_upright_and_the_hall_does_not() {
    let demo = Demo::new().expect("the worlds build");
    let gallery = demo.spot("gallery").expect("a gallery").room;
    let hall = demo.spot("hall").expect("a hall").room;
    assert_eq!(demo.up(gallery), Some(groxel::Face::PosY));
    assert_eq!(demo.up(hall), None);

    let mut me = groxel_body::Body::new(gallery, groxel::vec3(34.5, 5.0, 27.0));
    me.frame = groxel::Orientation::quarter_turn(0, 1);
    let still = groxel_body::Input::default();
    for _ in 0..240 {
        me.update(&demo.world, &still, 1.0 / 60.0);
        demo.right(&mut me);
    }
    assert_eq!(me.up_face(), groxel::Face::PosY);
    assert!(me.on_ground);
    assert!((me.pos[1] - 1.0).abs() < 1e-3, "{:?}", me.pos);
    // It came down near where it came in, not over by the mirror.
    assert!(me.pos[2] > 20.0, "{:?}", me.pos);

    // In the hall a body on the wall stays on the wall.
    let spot = demo.spot("hall").expect("a hall");
    let mut walker = Demo::body_at(&spot);
    walker.frame = groxel::Orientation::quarter_turn(2, 1);
    let turned = walker.up_face();
    assert!(demo.right(&mut walker));
    assert_eq!(walker.up_face(), turned);
}
