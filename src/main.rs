mod renderer;

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use mlua::{Function, IntoLuaMulti, Lua, MultiValue, UserData, UserDataFields, UserDataMethods};
use renderer::Renderer;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 240;
const WINDOW_SCALE: u32 = 3;
const BUFFER_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT;
const TARGET_FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / 60);

struct ScreenCoordinate {
    x: usize,
    y: usize,
}

impl ScreenCoordinate {
    fn to_index(&self) -> usize {
        (self.y * SCREEN_WIDTH) + self.x
    }
}

struct Screen {
    buffer: Box<[Color]>,
}

impl Screen {
    fn new() -> Self {
        Screen {
            buffer: vec![Color::BLACK; BUFFER_SIZE].into_boxed_slice(),
        }
    }

    fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            self.buffer[ScreenCoordinate { x, y }.to_index()] = color;
        }
    }

    fn get_pixel(&self, x: usize, y: usize) -> Option<Color> {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            Some(self.buffer[ScreenCoordinate { x, y }.to_index()])
        } else {
            None
        }
    }

    fn fill(&mut self, color: Color) {
        self.buffer.fill(color);
    }

    fn clear(&mut self) {
        self.fill(Color::BLACK);
    }
}

#[derive(Clone)]
struct SharedScreen(Rc<RefCell<Screen>>);

impl SharedScreen {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(Screen::new())))
    }
}

impl UserData for SharedScreen {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.0.borrow().width()));
        fields.add_field_method_get("height", |_, this| Ok(this.0.borrow().height()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "set_pixel",
            |_, this, (x, y, r, g, b, a): (isize, isize, u8, u8, u8, Option<u8>)| {
                if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
                    let color = Color::RGBA(r, g, b, a.unwrap_or(255));
                    this.0.borrow_mut().set_pixel(x, y, color);
                }
                Ok(())
            },
        );

        methods.add_method("get_pixel", |lua, this, (x, y): (isize, isize)| {
            if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y))
                && let Some(c) = this.0.borrow().get_pixel(x, y)
            {
                return (c.r, c.g, c.b, c.a).into_lua_multi(lua);
            }
            Ok(MultiValue::new())
        });

        methods.add_method("fill", |_, this, (r, g, b, a): (u8, u8, u8, Option<u8>)| {
            let color = Color::RGBA(r, g, b, a.unwrap_or(255));
            this.0.borrow_mut().fill(color);
            Ok(())
        });

        methods.add_method("clear", |_, this, ()| {
            this.0.borrow_mut().clear();
            Ok(())
        });
    }
}

struct ScriptEngine {
    _lua: Lua,
    setup_fn: Option<Function>,
    loop_fn: Option<Function>,
}

impl ScriptEngine {
    fn from_source(
        script_source: &str,
        script_name: &str,
        screen: SharedScreen,
    ) -> mlua::Result<Self> {
        let lua = Lua::new();
        lua.globals().set("screen", screen)?;

        lua.load(script_source).set_name(script_name).exec()?;

        let globals = lua.globals();
        let setup_fn = globals.get::<Option<Function>>("setup")?;
        let loop_fn = globals.get::<Option<Function>>("loop")?;

        Ok(Self {
            _lua: lua,
            setup_fn,
            loop_fn,
        })
    }

    fn load(script_path: &Path, screen: SharedScreen) -> Result<Self, Box<dyn Error>> {
        let script_source = fs::read_to_string(script_path).map_err(|err| {
            format!(
                "Failed to read Lua script '{}': {err}",
                script_path.display()
            )
        })?;

        Ok(Self::from_source(
            &script_source,
            &script_path.to_string_lossy(),
            screen,
        )?)
    }

    fn setup(&self) -> mlua::Result<()> {
        if let Some(setup_fn) = &self.setup_fn {
            setup_fn.call::<()>(())?;
        }
        Ok(())
    }

    fn r#loop(&self) -> mlua::Result<()> {
        if let Some(loop_fn) = &self.loop_fn {
            loop_fn.call::<()>(())?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let script_arg = env::args().nth(1).unwrap_or_else(|| "main.lua".to_string());
    let script_path = Path::new(&script_arg);

    let screen = SharedScreen::new();
    let engine = ScriptEngine::load(script_path, screen.clone())?;

    let sdl_context = sdl3::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window(
            "Fantasy Station - Classic Soft",
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

    'running: loop {
        let frame_start = Instant::now();

        for event in event_pump.poll_iter() {
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
                    renderer.window_mut().set_title(if classic_soft {
                        "Fantasy Station - Classic Soft"
                    } else {
                        "Fantasy Station - Pixel Perfect"
                    })?;
                }
                _ => {}
            }
        }

        // Endpoint "loop": runs every frame.
        engine.r#loop()?;

        // Render Screen state at the end of the loop.
        renderer.render(&screen.0.borrow().buffer, classic_soft)?;

        let elapsed = frame_start.elapsed();
        if elapsed < TARGET_FRAME_DURATION {
            thread::sleep(TARGET_FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_setup_and_loop_update_screen() {
        let screen = SharedScreen::new();
        let script = r#"
            local x = 0

            function setup()
                assert(screen.width == 320)
                assert(screen.height == 240)
                screen:fill(10, 20, 30)
                screen:set_pixel(5, 6, 255, 128, 64, 200)
            end

            function loop()
                screen:set_pixel(x, 10, 100, 150, 200)
                local r, g, b, a = screen:get_pixel(x, 10)
                assert(r == 100 and g == 150 and b == 200 and a == 255)
                -- Out-of-bounds calls should be safely ignored
                screen:set_pixel(-1, -1, 255, 0, 0)
                screen:set_pixel(999, 999, 255, 0, 0)
                assert(screen:get_pixel(-1, 0) == nil)
                x = x + 1
            end
        "#;

        let engine = ScriptEngine::from_source(script, "test.lua", screen.clone()).unwrap();

        engine.setup().unwrap();
        {
            let s = screen.0.borrow();
            assert_eq!(s.get_pixel(0, 0), Some(Color::RGBA(10, 20, 30, 255)));
            assert_eq!(s.get_pixel(5, 6), Some(Color::RGBA(255, 128, 64, 200)));
        }

        engine.r#loop().unwrap();
        engine.r#loop().unwrap();
        {
            let s = screen.0.borrow();
            assert_eq!(s.get_pixel(0, 10), Some(Color::RGBA(100, 150, 200, 255)));
            assert_eq!(s.get_pixel(1, 10), Some(Color::RGBA(100, 150, 200, 255)));
            assert_eq!(s.get_pixel(2, 10), Some(Color::RGBA(10, 20, 30, 255)));
        }
    }
}
