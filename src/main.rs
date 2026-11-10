mod disk;
mod input;
mod renderer;
mod screen;
mod script;

use std::env;
use std::error::Error;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use disk::load_disk;
use input::SharedInput;
use renderer::Renderer;
use screen::SharedScreen;
use script::ScriptEngine;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 240;
const WINDOW_SCALE: u32 = 3;
const TARGET_FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / 60);

fn main() -> Result<(), Box<dyn Error>> {
    let disk_arg = env::args()
        .nth(1)
        .unwrap_or_else(|| "disks/demo".to_string());
    let (disk, script_path) = load_disk(Path::new(&disk_arg))?;

    let screen = SharedScreen::new();
    let input = SharedInput::default();
    let engine = ScriptEngine::load(&script_path, screen.clone(), input.clone())?;

    let sdl_context = sdl3::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window(
            &format!("{} - Classic Soft", disk.name),
            (SCREEN_WIDTH as u32) * WINDOW_SCALE,
            (SCREEN_HEIGHT as u32) * WINDOW_SCALE,
        )
        .position_centered()
        .resizable()
        .build()?;

    let mut renderer = Renderer::new(window)?;
    let mut classic_soft = true;

    let mut event_pump = sdl_context.event_pump()?;

    // Endpoint "setup": runs once before entering the main loop.
    engine.setup()?;
    let mut gamepads = input::Gamepads::new(&sdl_context, &input);

    'running: loop {
        let frame_start = Instant::now();

        for event in event_pump.poll_iter() {
            gamepads.event(&event, &input);
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::P),
                    repeat: false,
                    ..
                } => {
                    classic_soft = !classic_soft;
                    let preset = if classic_soft {
                        "Classic Soft"
                    } else {
                        "Pixel Perfect"
                    };
                    renderer
                        .window_mut()
                        .set_title(&format!("{} - {preset}", disk.name))?;
                }
                _ => {}
            }
        }

        engine.frame()?;
        for player in input.0.borrow_mut().iter_mut() {
            player.end_frame();
        }

        // Render the completed frame.
        renderer.render(&screen.0.borrow().buffer, classic_soft)?;

        let elapsed = frame_start.elapsed();
        if elapsed < TARGET_FRAME_DURATION {
            thread::sleep(TARGET_FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}
