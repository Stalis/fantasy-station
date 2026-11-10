mod controller;
mod graphics;

use controller::LuaInput;
use graphics::LuaGraphics;

use std::error::Error;
use std::fs;
use std::path::Path;

use mlua::{Function, Lua};

use crate::gateway::{Graphics, Input};

pub(crate) struct ScriptEngine {
    _lua: Lua,
    setup_fn: Option<Function>,
    update_fn: Option<Function>,
    draw_fn: Option<Function>,
}

impl ScriptEngine {
    pub(crate) fn from_source(
        script_source: &str,
        script_name: &str,
        screen: Graphics,
        input: Input,
    ) -> mlua::Result<Self> {
        let lua = Lua::new();
        lua.globals().set("graphics", LuaGraphics(screen))?;
        lua.globals().set("input", LuaInput(input))?;

        lua.load(script_source).set_name(script_name).exec()?;

        let globals = lua.globals();
        let setup_fn = globals.get::<Option<Function>>("setup")?;
        let update_fn = globals.get::<Option<Function>>("update")?;
        let draw_fn = globals.get::<Option<Function>>("draw")?;

        Ok(Self {
            _lua: lua,
            setup_fn,
            update_fn,
            draw_fn,
        })
    }

    pub(crate) fn load(
        script_path: &Path,
        screen: Graphics,
        input: Input,
    ) -> Result<Self, Box<dyn Error>> {
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
            input,
        )?)
    }

    pub(crate) fn setup(&self) -> mlua::Result<()> {
        if let Some(setup_fn) = &self.setup_fn {
            setup_fn.call::<()>(())?;
        }
        Ok(())
    }

    pub(crate) fn frame(&self) -> mlua::Result<()> {
        if let Some(update_fn) = &self.update_fn {
            update_fn.call::<()>(())?;
        }
        if let Some(draw_fn) = &self.draw_fn {
            draw_fn.call::<()>(())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::Instant;

    use sdl3::pixels::Color;

    use crate::core::disk::load_disk;
    use crate::core::graphics::SharedScreen;
    use crate::core::input::{Button, SharedInput};

    #[test]
    fn disk_manifest_loads_entrypoint_and_rejects_invalid_paths() {
        let dir = env::temp_dir().join(format!(
            "fantasy-station-disk-{}-{:?}",
            std::process::id(),
            Instant::now()
        ));
        fs::create_dir_all(dir.join("scripts")).unwrap();
        fs::write(dir.join("scripts/start.lua"), "function update() end").unwrap();
        let manifest = dir.join("manifest.json");
        fs::write(
            &manifest,
            r#"{"name":"Example","entrypoint":"scripts/start.lua","future":true}"#,
        )
        .unwrap();
        let (disk, script) = load_disk(&dir).unwrap();
        assert_eq!(disk.name, "Example");
        assert_eq!(
            script,
            fs::canonicalize(dir.join("scripts/start.lua")).unwrap()
        );
        let engine = ScriptEngine::load(
            &script,
            Graphics::new(SharedScreen::new()),
            Input::new(SharedInput::default()),
        )
        .unwrap();
        engine.frame().unwrap();
        for invalid in [
            r#"{"name":"Example"}"#,
            r#"{"name":"","entrypoint":"scripts/start.lua"}"#,
            r#"{"name":"Example","entrypoint":"../outside.lua"}"#,
            r#"{"name":"Example","entrypoint":"/outside.lua"}"#,
            r#"{"name":"Example","entrypoint":"missing.lua"}"#,
            "not json",
        ] {
            fs::write(&manifest, invalid).unwrap();
            assert!(load_disk(&dir).is_err(), "accepted {invalid}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(env::temp_dir(), dir.join("outside")).unwrap();
            fs::write(&manifest, r#"{"name":"Example","entrypoint":"outside"}"#).unwrap();
            assert!(load_disk(&dir).is_err());
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lua_setup_update_and_draw_update_screen() {
        let screen = SharedScreen::new();
        let script = r#"
            local x = 0

            function setup()
                assert(graphics.width == 320)
                assert(graphics.height == 240)
                graphics:fill(0x001f)
                graphics:set_pixel(5, 6, 0x7c00)
            end

            function update()
                x = x + 1
            end

            function draw()
                graphics:set_pixel(x - 1, 10, 0x03e0)
                assert(graphics:get_pixel(x - 1, 10) == 0x03e0)
                graphics:set_pixel(-1, -1, 0x7c00)
                graphics:set_pixel(999, 999, 0x7c00)
                assert(graphics:get_pixel(-1, 0) == nil)
                graphics:rect(20, 20, 2, 2, 0x7fff)
                graphics:circle(30, 30, 1, 0x7c00)
                graphics:line(40, 40, 42, 40, 0x03e0)
                assert(graphics:get_pixel(21, 21) == 0x7fff)
                assert(graphics:get_pixel(30, 30) == 0x7c00)
                assert(graphics:get_pixel(42, 40) == 0x03e0)
                assert(not pcall(function() graphics:fill(0x8000) end))
            end
        "#;

        let engine = ScriptEngine::from_source(
            script,
            "test.lua",
            Graphics::new(screen.clone()),
            Input::new(SharedInput::default()),
        )
        .unwrap();

        engine.setup().unwrap();
        {
            let s = screen.0.borrow();
            assert_eq!(s.get_pixel(0, 0), Some(Color::RGBA(0, 0, 255, 255)));
            assert_eq!(s.get_pixel(5, 6), Some(Color::RGBA(255, 0, 0, 255)));
        }

        screen.0.borrow_mut().begin_frame();
        engine.frame().unwrap();
        screen.0.borrow_mut().publish_frame();
        {
            let s = screen.0.borrow();
            assert_eq!(s.displayed_rows().nth(6).unwrap()[5], 0x7c00);
            assert_eq!(s.displayed_rows().nth(10).unwrap()[0], 0x03e0);
        }
        screen.0.borrow_mut().begin_frame();
        engine.frame().unwrap();
        screen.0.borrow_mut().publish_frame();
        {
            let s = screen.0.borrow();
            let row = s.displayed_rows().nth(10).unwrap();
            assert_eq!(&row[..3], &[0x03e0, 0x03e0, 0x001f]);
        }
        {
            let s = screen.0.borrow();
            assert_eq!(s.get_pixel(0, 10), Some(Color::RGBA(0, 255, 0, 255)));
            assert_eq!(s.get_pixel(1, 10), Some(Color::RGBA(0, 255, 0, 255)));
            assert_eq!(s.get_pixel(2, 10), Some(Color::RGBA(0, 0, 255, 255)));
        }
    }

    #[test]
    fn failed_callbacks_do_not_publish_partial_frames() {
        for callback in ["update", "draw"] {
            let screen = SharedScreen::new();
            let script = format!(
                "function setup() graphics:set_pixel(0, 0, 0x7c00) end\n\
                 function {callback}() graphics:set_pixel(0, 0, 0x03e0); error('failed') end"
            );
            let engine = ScriptEngine::from_source(
                &script,
                "test.lua",
                Graphics::new(screen.clone()),
                Input::new(SharedInput::default()),
            )
            .unwrap();
            engine.setup().unwrap();
            screen.0.borrow_mut().begin_frame();
            assert!(engine.frame().is_err());
            assert_eq!(
                screen.0.borrow().displayed_rows().next().unwrap()[0],
                0x7c00
            );
        }
    }

    #[test]
    fn missing_frame_callbacks_still_publish_setup() {
        let screen = SharedScreen::new();
        let engine = ScriptEngine::from_source(
            "function setup() graphics:set_pixel(0, 0, 0x7c00) end",
            "test.lua",
            Graphics::new(screen.clone()),
            Input::new(SharedInput::default()),
        )
        .unwrap();
        engine.setup().unwrap();
        screen.0.borrow_mut().begin_frame();
        engine.frame().unwrap();
        screen.0.borrow_mut().publish_frame();
        assert_eq!(
            screen.0.borrow().displayed_rows().next().unwrap()[0],
            0x7c00
        );
    }

    #[test]
    fn lua_released_observes_non_consuming_frame_edges() {
        let input = SharedInput::default();
        let script = r#"
            local first = input:get_controller()
            local second = input:get_controller(1)
            local frame = 0
            function setup()
                assert(not first:released("left"))
                local ok, err = pcall(function() return first:released("unknown") end)
                assert(not ok and string.find(tostring(err), "unknown input button"))
            end
            function update()
                frame = frame + 1
                assert(first:released("left") == (frame == 1))
                assert(first:released("left") == (frame == 1))
                assert(not first:down("left"))
                assert(not second:released("left"))
            end
            function draw()
                assert(first:released("left") == (frame == 1))
            end
        "#;
        let engine = ScriptEngine::from_source(
            script,
            "released.lua",
            Graphics::new(SharedScreen::new()),
            Input::new(input.clone()),
        )
        .unwrap();
        engine.setup().unwrap();
        input.0.borrow_mut()[0].set(Button::Left, false, true);
        input.0.borrow_mut()[0].set(Button::Left, false, false);
        engine.frame().unwrap();
        input.0.borrow_mut()[0].end_frame();
        engine.frame().unwrap();
    }

    #[test]
    fn lua_input_observes_edges_across_frames() {
        let input = SharedInput::default();
        let script = r#"
            local first = input:get_controller()
            local explicit_first = input:get_controller(0)
            local second = input:get_controller(1)
            function setup()
                assert(not first:down("left") and not first:pressed("left"))
                local ok, err = pcall(function() return first:down("invalid") end)
                assert(not ok and string.find(tostring(err), "unknown input button: invalid"))
                for _, index in ipairs({2, 3, -1}) do
                    local valid, message = pcall(function() return input:get_controller(index) end)
                    assert(not valid and string.find(tostring(message), "expected 0 or 1"))
                end
                assert(not second:down("left"))
            end
            local frame = 0
            function update()
                frame = frame + 1
                assert(first:down("left"))
                assert(explicit_first:down("left"))
                assert(second:down("south"))
                assert(not first:down("south"))
                assert(second:pressed("south") == (frame == 1))
                assert(first:pressed("left") == (frame == 1))
                assert(first:pressed("left") == (frame == 1))
            end
        "#;
        let engine = ScriptEngine::from_source(
            script,
            "input.lua",
            Graphics::new(SharedScreen::new()),
            Input::new(input.clone()),
        )
        .unwrap();
        engine.setup().unwrap();
        input.0.borrow_mut()[0].set(Button::Left, false, true);
        input.0.borrow_mut()[1].set(Button::South, true, true);
        engine.frame().unwrap();
        input.0.borrow_mut()[0].end_frame();
        input.0.borrow_mut()[1].end_frame();
        engine.frame().unwrap();
    }
}
