//! Walk around the impossible worlds.
//!
//! ```text
//! cargo run --release -p groxel-walk [-- width height]
//! ```
//!
//! | Key | Does |
//! |---|---|
//! | click | capture the mouse; then left click places a glowing marker and right click removes a voxel |
//! | W A S D, mouse | move, look |
//! | space, shift | jump, run |
//! | F | fly on or off; Q and E go down and up while flying |
//! | 1 to 9, tab | go to a world, or to the next one |
//! | P | save a screenshot |
//! | esc | release the mouse; again to quit |
//!
//! The picture is rendered at `width` by `height` (640 by 360 unless given)
//! and scaled to the window.

use groxel::{HitKind, Ray, TraceOpts, trace};
use groxel_app::{Demo, camera, mat};
use groxel_body::{Body, Input};
use groxel_render::{Renderer, Settings, write_png};
use std::collections::HashSet;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

/// Radians of turn for one unit of mouse movement.
const MOUSE: f32 = 0.0025;
/// How far the centre ray reaches when placing or removing a voxel.
const REACH: f32 = 60.0;

const DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

struct Shown {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
}

struct App {
    demo: Demo,
    renderer: Renderer,
    me: Body,
    spot: usize,
    /// How far the viewer has walked, which sets the stride of its figure.
    odometer: f32,
    /// The rendered picture, `0xAABBGGRR` a pixel.
    pixels: Vec<u32>,

    shown: Option<Shown>,
    keys: HashSet<KeyCode>,
    /// Mouse movement since the last frame.
    mouse: (f64, f64),
    grabbed: bool,
    /// Things asked for since the last frame.
    place: bool,
    remove: bool,
    screenshot: bool,
    screenshots: u32,
    last: Option<Instant>,
    /// Frames and seconds since the title was last written.
    frames: u32,
    since_title: f32,
}

impl App {
    fn new(width: u32, height: u32) -> App {
        let demo = Demo::new().expect("the demo worlds fit together");
        let mut renderer = Renderer::new(Settings {
            width,
            height,
            ..Default::default()
        });
        demo.paint(&mut renderer);
        let me = Demo::body_at(&demo.spots[0]);
        App {
            demo,
            renderer,
            me,
            spot: 0,
            odometer: 0.0,
            pixels: vec![0; (width * height) as usize],
            shown: None,
            keys: HashSet::new(),
            mouse: (0.0, 0.0),
            grabbed: false,
            place: false,
            remove: false,
            screenshot: false,
            screenshots: 0,
            last: None,
            frames: 0,
            since_title: 0.0,
        }
    }

    fn grab(&mut self, on: bool) {
        let Some(shown) = &self.shown else { return };
        let ok = if on {
            // Not every platform can lock the pointer in place; keeping it
            // inside the window is the next best thing.
            shown.window.set_cursor_grab(CursorGrabMode::Locked).is_ok()
                || shown
                    .window
                    .set_cursor_grab(CursorGrabMode::Confined)
                    .is_ok()
        } else {
            shown.window.set_cursor_grab(CursorGrabMode::None).is_ok()
        };
        if ok {
            shown.window.set_cursor_visible(!on);
            self.grabbed = on;
        }
    }

    fn go_to(&mut self, spot: usize) {
        if spot < self.demo.spots.len() {
            self.spot = spot;
            self.me = Demo::body_at(&self.demo.spots[spot]);
        }
    }

    /// A key went down. Held keys are read from `keys` each frame; this is
    /// for the keys that act once.
    fn pressed(&mut self, code: KeyCode, event_loop: &ActiveEventLoop) {
        match code {
            KeyCode::Escape if self.grabbed => self.grab(false),
            KeyCode::Escape => event_loop.exit(),
            KeyCode::KeyF => self.me.fly = !self.me.fly,
            KeyCode::KeyP => self.screenshot = true,
            KeyCode::Tab => self.go_to((self.spot + 1) % self.demo.spots.len()),
            _ => {
                if let Some(i) = DIGITS.iter().position(|&digit| digit == code) {
                    self.go_to(i);
                }
            }
        }
    }

    /// Advance the world by `dt` seconds and render it into `pixels`.
    fn step(&mut self, dt: f32) {
        let held = |code| self.keys.contains(&code);
        let axis = |plus, minus| held(plus) as i32 as f32 - held(minus) as i32 as f32;
        let (mx, my) = std::mem::take(&mut self.mouse);
        let input = Input {
            forward: axis(KeyCode::KeyW, KeyCode::KeyS),
            strafe: axis(KeyCode::KeyD, KeyCode::KeyA),
            rise: axis(KeyCode::KeyE, KeyCode::KeyQ),
            turn: -(mx as f32) * MOUSE,
            look: -(my as f32) * MOUSE,
            jump: held(KeyCode::Space),
            run: held(KeyCode::ShiftLeft) || held(KeyCode::ShiftRight),
        };

        let (before, crossings) = (self.me.pos, self.me.crossings);
        self.demo.tick(dt);
        self.me.update(&self.demo.world, &input, dt);
        // A room with a floor stands you upright if you arrive turned.
        self.demo.right(&mut self.me);
        // A seam changes the frame the position is written in, so a step
        // that crossed one is left out of the distance walked.
        if self.me.crossings == crossings {
            self.odometer += (self.me.pos - before).length();
        }

        let view = self.me.view(&self.demo.world);
        let (place, remove) = (
            std::mem::take(&mut self.place),
            std::mem::take(&mut self.remove),
        );
        if place || remove {
            // The centre ray says where.
            let ray = Ray {
                start: view.cell,
                origin: view.pos,
                dir: view.fwd,
            };
            let opts = TraceOpts {
                max_t: REACH,
                ..Default::default()
            };
            let hit = trace(&self.demo.world, ray, &opts);
            if hit.kind == HitKind::Solid {
                if place {
                    let room = self.demo.world.room_mut(hit.from.room);
                    room.set(hit.from.at, mat::MARKER);
                } else {
                    self.demo.world.room_mut(hit.cell.room).set(hit.cell.at, 0);
                }
                self.demo.relight();
            }
        }

        let stamped = self.demo.stamp(Some(&self.me), self.odometer, true);
        let frame = self.demo.frame(&stamped);
        self.renderer
            .render(&frame, &camera(&view), &mut self.pixels);

        if std::mem::take(&mut self.screenshot) {
            self.screenshots += 1;
            let name = format!("groxel-{:03}.png", self.screenshots);
            let s = self.renderer.settings;
            match write_png(&name, s.width, s.height, &self.pixels) {
                Ok(()) => println!("saved {name}"),
                Err(why) => eprintln!("cannot save {name}: {why}"),
            }
        }
    }

    /// Scale the picture to the window, nearest pixel, and show it.
    fn present(&mut self) {
        let Some(shown) = &mut self.shown else { return };
        let size = shown.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if shown.surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buffer) = shown.surface.buffer_mut() else {
            return;
        };
        let s = self.renderer.settings;
        let (dw, dh) = (size.width as usize, size.height as usize);
        let (sw, sh) = (s.width as usize, s.height as usize);
        for (y, row) in buffer.chunks_mut(dw).enumerate() {
            let from = &self.pixels[(y * sh / dh) * sw..][..sw];
            for (x, out) in row.iter_mut().enumerate() {
                let p = from[x * sw / dw];
                // The renderer's pixel is 0xAABBGGRR; the window's is
                // 0x00RRGGBB.
                *out = (p & 0xFF) << 16 | (p & 0xFF00) | (p >> 16 & 0xFF);
            }
        }
        // A frame that cannot be shown is dropped; the next one tries again.
        let _ = buffer.present();
    }

    fn title(&mut self, dt: f32) {
        self.frames += 1;
        self.since_title += dt;
        if self.since_title < 0.5 {
            return;
        }
        if let Some(shown) = &self.shown {
            let ms = 1000.0 * self.since_title / self.frames as f32;
            let name = self.demo.spots[self.spot].name;
            let mode = if self.me.fly { "flying" } else { "walking" };
            let hint = if self.grabbed { "" } else { " | click to look" };
            shown
                .window
                .set_title(&format!("groxel | {name} | {mode} | {ms:.0} ms{hint}"));
        }
        self.frames = 0;
        self.since_title = 0.0;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_some() {
            return;
        }
        let s = self.renderer.settings;
        let scale = (1280 / s.width).max(1);
        let attrs = Window::default_attributes()
            .with_title("groxel")
            .with_inner_size(LogicalSize::new(s.width * scale, s.height * scale));
        let window = Rc::new(event_loop.create_window(attrs).expect("a window"));
        let context = softbuffer::Context::new(window.clone()).expect("a display");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("a surface");
        window.request_redraw();
        self.shown = Some(Shown { window, surface });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                if event.state == ElementState::Released {
                    self.keys.remove(&code);
                } else if self.keys.insert(code) {
                    self.pressed(code, event_loop);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                if !self.grabbed {
                    self.grab(true);
                } else if button == MouseButton::Left {
                    self.place = true;
                } else if button == MouseButton::Right {
                    self.remove = true;
                }
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.grab(false);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                // A long pause (a dragged window, a debugger) must not turn
                // into one long step.
                let dt = self
                    .last
                    .map_or(0.0, |last| (now - last).as_secs_f32().min(0.05));
                self.last = Some(now);
                self.step(dt);
                self.present();
                self.title(dt);
                if let Some(shown) = &self.shown {
                    shown.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.grabbed {
                self.mouse.0 += delta.0;
                self.mouse.1 += delta.1;
            }
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1).map(|arg| arg.parse::<u32>());
    let (width, height) = match (args.next(), args.next()) {
        (None, _) => (640, 360),
        (Some(Ok(w)), Some(Ok(h))) if w > 0 && h > 0 => (w, h),
        _ => {
            eprintln!("usage: groxel-walk [width height]");
            std::process::exit(2);
        }
    };
    let event_loop = EventLoop::new().expect("an event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop
        .run_app(&mut App::new(width, height))
        .expect("the event loop runs");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything but the window: held keys move the viewer, the mouse turns
    /// it, a frame is rendered, and a placed marker changes the world.
    #[test]
    fn a_step_moves_looks_renders_and_edits_without_a_window() {
        let mut app = App::new(64, 36);
        let start = app.me;
        app.keys.insert(KeyCode::KeyW);
        app.mouse = (40.0, 0.0);
        for _ in 0..30 {
            app.step(1.0 / 60.0);
        }
        assert!((app.me.pos - start.pos).length() > 1.0);
        assert!(app.odometer > 1.0);
        assert!((app.me.yaw - (start.yaw - 40.0 * MOUSE)).abs() < 1e-5);
        assert!(app.pixels.iter().all(|&p| p >> 24 == 0xFF));
        assert!(app.pixels.iter().any(|&p| p != app.pixels[0]));

        let before = app.demo.world.clone();
        app.place = true;
        app.step(1.0 / 60.0);
        let changed = (0..before.rooms().len()).any(|i| {
            let id = groxel::RoomId(i as u32);
            let dims = before.room(id).dims();
            (0..dims[2]).any(|z| {
                (0..dims[1]).any(|y| {
                    (0..dims[0]).any(|x| {
                        let at = groxel::ivec3(x, y, z);
                        before.room(id).material(at) != app.demo.world.room(id).material(at)
                    })
                })
            })
        });
        assert!(changed, "the centre ray should have found a wall to mark");

        // Tab and the digits move between the named places.
        app.go_to(2);
        assert_eq!(app.spot, 2);
        app.go_to(99);
        assert_eq!(app.spot, 2);
    }
}
