//! The headless camera: its arguments, how it is put in place, and what its
//! pictures show.

use groxel_app::Demo;
use groxel_app::shot::{BadArgument, Shot};
use groxel_body::Input;
use groxel_render::Settings;
use std::f32::consts::{FRAC_PI_2, PI};
use std::process::Command;
use std::sync::LazyLock;

/// Building the worlds floods six million voxels with light, so the tests
/// share one demo. A shot runs the world forward, so each takes a copy.
static DEMO: LazyLock<Demo> = LazyLock::new(|| Demo::new().expect("the worlds fit together"));

const DT: f32 = 1.0 / 60.0;

fn shot(args: &[&str]) -> Shot {
    let mut shot = Shot::default();
    shot.parse(args).expect("arguments the camera knows");
    shot
}

/// A small picture, on one thread.
fn small(args: &[&str]) -> Shot {
    let mut all = vec!["w=96", "h=54", "threads=1"];
    all.extend(args);
    shot(&all)
}

fn rgb(pixel: u32) -> [i32; 3] {
    let b = pixel.to_le_bytes();
    [b[0] as i32, b[1] as i32, b[2] as i32]
}

// --- Arguments -------------------------------------------------------------

#[test]
fn the_default_shot_is_two_seconds_in_at_960_by_540() {
    let want = Shot {
        // Four samples a pixel; everything else as the renderer has it.
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
    };
    assert_eq!(Shot::default(), want);
    assert_eq!(shot(&[]), want);
    let set = want.settings;
    assert_eq!(
        (set.fov, set.shadows, set.ao, set.ink),
        (80.0, true, true, 0.5)
    );
}

#[test]
fn arguments_set_the_shot() {
    let s = shot(&[
        "w=320",
        "h=180",
        "ss=3",
        "fov=70",
        "shadows=0",
        "ao=0",
        "ink=0.25",
        "levels=4",
        "threads=2",
        "dx=-6",
        "dy=1.5",
        "dz=6",
        "yaw=150",
        "pitch=-10",
        "t=3",
        "me=0",
        "walk=4",
        "fly=1",
    ]);
    let set = s.settings;
    assert_eq!((set.width, set.height, set.samples), (320, 180, 3));
    assert_eq!((set.fov, set.ink), (70.0, 0.25));
    assert_eq!((set.shadows, set.ao), (false, false));
    assert_eq!((set.levels, set.threads), (4, 2));
    assert_eq!((s.dx, s.dy, s.dz), (-6.0, 1.5, 6.0));
    // Angles are given in degrees and kept in radians.
    assert_eq!(s.yaw, Some(150.0 * PI / 180.0));
    assert_eq!(s.pitch, Some(-10.0 * PI / 180.0));
    assert_eq!((s.t, s.me, s.walk, s.fly), (3.0, false, 4.0, true));

    // A later argument replaces an earlier one, and what is not named
    // stays as it was.
    let s = shot(&["t=1", "t=5", "shadows=2"]);
    assert_eq!((s.t, s.settings.shadows), (5.0, true));
    assert_eq!(s.settings.width, 960);
    assert_eq!(s.yaw, None);
}

#[test]
fn arguments_that_mean_nothing_are_refused() {
    for bad in [
        "w", "w=", "w=wide", "=3", "zoom=2", "W=320", "t = 3", "w=0", "h=-4",
    ] {
        let mut s = Shot::default();
        let err = s.parse(&["t=7", bad, "dx=1"]).unwrap_err();
        assert_eq!(err, BadArgument(bad.to_owned()));
        let said = err.to_string();
        assert!(said.contains(bad) && said.contains("key=number"), "{said}");
        // Arguments are taken in order, up to the one that is wrong.
        assert_eq!((s.t, s.dx), (7.0, 0.0), "{bad}");
    }
    // Every key the message lists is one the camera knows.
    for key in Shot::KEYS.split(' ') {
        assert!(
            Shot::default().parse(&[format!("{key}=1")]).is_ok(),
            "{key}"
        );
    }
    assert_eq!(Shot::KEYS.split(' ').count(), 18);
}

// --- Putting the camera in place -------------------------------------------

#[test]
fn the_world_runs_for_the_time_asked_on_a_single_precision_clock() {
    let spot = DEMO.spot("gallery").unwrap();
    // The clock adds a sixtieth of a second in single precision until it is
    // not less than the time asked. A sixtieth is not exact, so whole
    // seconds take one tick more than sixty a second.
    for (t, ticks) in [(0.0, 0), (0.5, 30), (1.0, 61), (2.0, 121), (3.0, 181)] {
        let mut staged = DEMO.clone();
        let s = Shot {
            t,
            ..Default::default()
        };
        s.stage(&mut staged, &spot);

        let mut ticked = DEMO.clone();
        for _ in 0..ticks {
            ticked.tick(DT);
        }
        assert_eq!(staged.walkers, ticked.walkers, "t = {t}");
        if ticks > 0 {
            // One tick fewer is a different world.
            let mut short = DEMO.clone();
            for _ in 0..ticks - 1 {
                short.tick(DT);
            }
            assert_ne!(staged.walkers, short.walkers, "t = {t}");
        }
    }
    assert_eq!(Shot::default().t, 2.0);
}

#[test]
fn the_camera_is_moved_and_turned_then_left_to_settle() {
    let mut d = DEMO.clone();
    let spot = d.spot("hall").unwrap();
    let s = shot(&["dx=2", "dy=3", "dz=4", "yaw=90", "pitch=10", "t=1"]);

    let start = s.start(&spot);
    assert_eq!(start.pos.0, [12.5, 4.0, 9.0]);
    assert_eq!(start.yaw, FRAC_PI_2);
    // The pitch is the spot's until the camera has settled.
    assert_eq!(start.pitch, spot.pitch);
    assert!(!start.on_ground);

    let b = s.stage(&mut d, &spot);
    // It was put three voxels up, and has dropped to the floor.
    assert_eq!(b.pos.0, [12.5, 1.0, 9.0]);
    assert!(b.on_ground);
    assert_eq!((b.yaw, b.pitch), (FRAC_PI_2, 10.0 * PI / 180.0));
    assert_eq!(b.eye_lag, 0.0);
    // With no pitch given, the spot's is kept.
    let level = shot(&["t=0"]).stage(&mut d, &spot);
    assert_eq!(level.pitch, spot.pitch);
    assert_eq!(level.pos, spot.pos);
}

#[test]
fn a_camera_in_the_air_falls_unless_it_flies() {
    let mut d = DEMO.clone();
    // The overlook is a spot in mid air, and flies.
    let overlook = d.spot("overlook").unwrap();
    let b = shot(&["dy=-20", "t=4"]).stage(&mut d, &overlook);
    assert_eq!(b.pos, overlook.pos + groxel::vec3(0.0, -20.0, 0.0));
    assert!(b.fly && !b.on_ground);

    // Twenty-two voxels along the well's south stair from its spot, the
    // tread is six voxels lower. Half a second is not long enough to land.
    let well = d.spot("well").unwrap();
    let falling = shot(&["dx=22", "t=0.5"]).stage(&mut DEMO.clone(), &well);
    assert!(!falling.on_ground && falling.vel_up < -20.0);
    assert!(
        falling.pos[1] > 22.5 && falling.pos[1] < 22.7,
        "{:?}",
        falling.pos
    );
    let landed = shot(&["dx=22", "t=2"]).stage(&mut DEMO.clone(), &well);
    assert!(landed.on_ground);
    assert_eq!(landed.pos.0, [27.5, 22.0, 5.0]);
    // Told to fly, it stays where it was put.
    let hung = shot(&["dx=22", "t=2", "fly=1"]).stage(&mut DEMO.clone(), &well);
    assert_eq!(hung.pos.0, [27.5, 28.0, 5.0]);
}

#[test]
fn walking_first_takes_the_camera_forward_and_the_world_with_it() {
    let mut d = DEMO.clone();
    let spot = d.spot("hall").unwrap();
    // Ten voxels a second: ten voxels is a second's walk, down the hall.
    let b = shot(&["t=0", "walk=10"]).stage(&mut d, &spot);
    assert!((b.pos[2] - (spot.pos[2] + 10.0)).abs() < 0.2, "{:?}", b.pos);
    assert_eq!((b.pos[0], b.pos[1]), (spot.pos[0], spot.pos[1]));
    // The walkers had that second too: each has gone its own speed's worth.
    for k in &d.walkers {
        let gone = k.odometer / k.body.walk_speed;
        assert!((gone - 1.0).abs() < 0.02, "{gone}");
    }
}

#[test]
fn the_eye_is_not_left_easing_over_the_last_step() {
    let mut d = DEMO.clone();
    let spot = d.spot("well").unwrap();
    // East along the well's south stair, which goes down a voxel every
    // three. The eye lags behind each drop and catches up over a few ticks.
    let s = shot(&["t=0", "yaw=90", "walk=14"]);
    let b = s.stage(&mut d, &spot);
    assert!(b.on_ground);
    assert!(b.pos[0] > spot.pos[0] + 13.0, "{:?}", b.pos);
    assert!(b.pos[1] <= spot.pos[1] - 3.0, "{:?}", b.pos);
    assert_eq!(b.eye_lag, 0.0);

    // The same walk by hand ends in the same place with the eye still
    // easing, so the picture would be taken from a little too high.
    let k = &d.walkers[0];
    let ticks = (k.odometer / (k.body.walk_speed * DT)).round() as usize;
    let forward = Input {
        forward: 1.0,
        ..Default::default()
    };
    let mut by_hand = s.start(&spot);
    for _ in 0..ticks {
        by_hand.update(&DEMO.world, &forward, DT);
    }
    assert_eq!(by_hand.pos, b.pos);
    assert!(by_hand.eye_lag > 0.01, "{}", by_hand.eye_lag);
    let (settled, easing) = (b.view(&DEMO.world), by_hand.view(&DEMO.world));
    assert!(easing.pos[1] > settled.pos[1] + 0.01);
}

#[test]
#[should_panic(expected = "inside its room")]
fn a_camera_put_outside_its_room_is_refused() {
    let mut d = DEMO.clone();
    let spot = d.spot("gallery").unwrap();
    shot(&["dz=-40"]).stage(&mut d, &spot);
}

// --- Pictures --------------------------------------------------------------

#[test]
fn a_still_is_the_size_asked_and_the_same_every_time() {
    let spot = DEMO.spot("well").unwrap();
    let s = small(&["t=0.5"]);
    let first = s.take(&mut DEMO.clone(), &spot);
    assert_eq!(first.len(), 96 * 54);
    assert!(first.iter().all(|p| p >> 24 == 0xFF));
    assert!(first.iter().any(|p| p & 0xFF_FFFF != first[0] & 0xFF_FFFF));
    // On every core, too.
    let threaded = shot(&["w=96", "h=54", "t=0.5"]);
    assert_eq!(threaded.take(&mut DEMO.clone(), &spot), first);
    // Staging and drawing in two steps is the same as `take`.
    let mut d = DEMO.clone();
    let body = s.stage(&mut d, &spot);
    assert_eq!(s.render(&d, &body), first);
}

#[test]
fn the_gallery_still_shows_its_doors_the_right_way_round() {
    let spot = DEMO.spot("gallery").unwrap();
    let px = small(&["t=3"]).take(&mut DEMO.clone(), &spot);
    let at = |x: usize, y: usize| rgb(px[y * 96 + x]);
    // Facing the doors, +x is to the left: the blue frame of the hall's door
    // is on the left, the gold of the tower's in the middle, the red of the
    // well's on the right. Each is sampled on the bar above its opening.
    let [r, g, b] = at(18, 19);
    assert!(b > r + 20 && b > g, "left frame is {:?}", [r, g, b]);
    let [r, g, b] = at(48, 19);
    assert!(r > b + 60 && g > b + 30, "middle frame is {:?}", [r, g, b]);
    let [r, g, b] = at(77, 19);
    assert!(r > 2 * g && r > 2 * b, "right frame is {:?}", [r, g, b]);
    // The checkered floor is below, lit plaster above.
    let [r, g, b] = at(48, 4);
    assert!(r > 150 && g > 120 && b > 80, "wall is {:?}", [r, g, b]);
}

#[test]
fn you_are_in_the_mirror_and_only_there() {
    let spot = DEMO.spot("gallery").unwrap();
    // Facing the mirror, your own figure is in the glass.
    let with = small(&["yaw=180"]).take(&mut DEMO.clone(), &spot);
    let without = small(&["yaw=180", "me=0"]).take(&mut DEMO.clone(), &spot);
    let blue = |px: &[u32]| {
        px.iter()
            .map(|&p| rgb(p))
            .filter(|[r, g, b]| *b > *r + 25 && *b > *g + 10)
            .count()
    };
    assert_ne!(with, without);
    // The glass also shows the blue frame of the hall's door, behind you.
    assert!(
        blue(&with) > blue(&without) + 100,
        "{} against {}",
        blue(&with),
        blue(&without)
    );
    // Just above the middle of the picture is your own hood.
    let at = 24 * 96 + 50;
    let [r, g, b] = rgb(with[at]);
    assert!(b > r && b > g, "{:?}", [r, g, b]);
    assert_ne!(with[at], without[at]);

    // Facing the doors, no seam shows you to yourself, and your own figure
    // is not drawn although the eye is inside it.
    let with = small(&[]).take(&mut DEMO.clone(), &spot);
    let without = small(&["me=0"]).take(&mut DEMO.clone(), &spot);
    assert_eq!(with, without);
}

// --- The program -----------------------------------------------------------

fn still(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_still"))
        .args(args)
        .output()
        .expect("the still program runs")
}

#[test]
fn the_program_writes_a_png_and_says_how_long_it_took() {
    let path = std::env::temp_dir().join(format!("groxel-still-{}.png", std::process::id()));
    let out = still(&[
        "hall",
        path.to_str().unwrap(),
        "w=64",
        "h=36",
        "ss=1",
        "t=0.5",
    ]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{said}");
    assert!(
        said.contains("64 x 36") && said.contains("rendered in"),
        "{said}"
    );

    let png = std::fs::read(&path).expect("the picture was written");
    std::fs::remove_file(&path).ok();
    assert_eq!(png[..8], [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    // The header chunk holds the width and the height.
    assert_eq!(png[12..16], *b"IHDR");
    assert_eq!(png[16..24], [0, 0, 0, 64, 0, 0, 0, 36]);

    // The same picture as the library makes.
    let spot = DEMO.spot("hall").unwrap();
    let want = shot(&["w=64", "h=36", "ss=1", "t=0.5"]).take(&mut DEMO.clone(), &spot);
    // Rows of a filter byte and three bytes a pixel, in stored blocks: the
    // first row starts after the signature, the header chunk, the data
    // chunk's own header, two bytes of zlib header and five of block header.
    let row = 8 + 25 + 8 + 2 + 5;
    assert_eq!(png[row], 0);
    for (x, p) in want[..64].iter().enumerate() {
        let at = row + 1 + 3 * x;
        assert_eq!(png[at..at + 3], p.to_le_bytes()[..3], "pixel {x}");
    }
}

#[test]
fn the_program_explains_what_it_cannot_do() {
    // If a run that should fail were to write its picture, it would go here.
    let path = std::env::temp_dir().join(format!("groxel-unused-{}.png", std::process::id()));
    let out = path.to_str().unwrap();
    let said = |args: &[&str]| {
        let run = still(args);
        assert!(!run.status.success(), "{args:?} should fail");
        assert!(run.stdout.is_empty(), "{args:?}");
        String::from_utf8_lossy(&run.stderr).into_owned()
    };

    assert!(said(&[]).contains("usage: still <spot> <out.png>"));
    assert!(said(&["hall"]).contains("usage: still <spot> <out.png>"));
    let nowhere = said(&["cellar", out]);
    let spots = "no spot called 'cellar'. Spots: gallery well tower overlook hall";
    assert!(nowhere.contains(spots), "{nowhere}");
    let bad = said(&["hall", out, "zoom=2"]);
    assert!(bad.contains("cannot use 'zoom=2'"), "{bad}");
    let outside = said(&["hall", out, "dx=500"]);
    assert!(
        outside.contains("outside the hall (22 x 22 x 56 voxels)"),
        "{outside}"
    );

    let written = path.exists();
    std::fs::remove_file(&path).ok();
    assert!(!written, "a run that failed wrote a picture");
}
