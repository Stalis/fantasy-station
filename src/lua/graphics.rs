use mlua::{AnyUserData, UserData, UserDataFields, UserDataMethods, Value};

use crate::gateway::{Graphics, Sprite, Texture};

pub(super) struct LuaGraphics(pub(super) Graphics);
struct LuaTexture(Texture);
struct LuaSprite(Sprite);

impl UserData for LuaSprite {}

macro_rules! add_drawing_methods {
    ($methods:ident) => {
        $methods.add_method("set_pixel", |_, this, (x, y, color): (i64, i64, i64)| {
            this.0.set_pixel(x, y, color).map_err(mlua::Error::external)
        });
        $methods.add_method("get_pixel", |_, this, (x, y): (i64, i64)| Ok(this.0.get_pixel(x, y)));
        $methods.add_method("fill", |_, this, color: i64| this.0.fill(color).map_err(mlua::Error::external));
        $methods.add_method("clear", |_, this, ()| { this.0.clear(); Ok(()) });
        $methods.add_method("rect", |_, this, (x, y, width, height, color): (i64, i64, i64, i64, i64)| {
            this.0.rect(x, y, width, height, color).map_err(mlua::Error::external)
        });
        $methods.add_method("circle", |_, this, (x, y, radius, color): (i64, i64, i64, i64)| {
            this.0.circle(x, y, radius, color).map_err(mlua::Error::external)
        });
        $methods.add_method("rect_texture", |_, this, (x, y, width, height, source, mode): (i64, i64, i64, i64, AnyUserData, Value)| {
            let Value::String(mode) = mode else {
                return Err(mlua::Error::external("mapping must be 'stretch' or 'repeat'"));
            };
            let source = source.borrow::<LuaTexture>()?;
            this.0.rect_texture(x, y, width, height, &source.0, mode.to_str()?.as_ref()).map_err(mlua::Error::external)
        });
        $methods.add_method("circle_texture", |_, this, (x, y, radius, source, mode): (i64, i64, i64, AnyUserData, Value)| {
            let Value::String(mode) = mode else {
                return Err(mlua::Error::external("mapping must be 'stretch' or 'repeat'"));
            };
            let source = source.borrow::<LuaTexture>()?;
            this.0.circle_texture(x, y, radius, &source.0, mode.to_str()?.as_ref()).map_err(mlua::Error::external)
        });
        $methods.add_method("draw_sprite", |_, this, (sprite, x, y): (AnyUserData, i64, i64)| {
            let sprite = sprite.borrow::<LuaSprite>()?;
            this.0.draw_sprite(&sprite.0, x, y).map_err(mlua::Error::external)
        });
        $methods.add_method("line", |_, this, (x0, y0, x1, y1, color): (i64, i64, i64, i64, i64)| {
            this.0.line(x0, y0, x1, y1, color).map_err(mlua::Error::external)
        });
    };
}

impl UserData for LuaGraphics {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.0.width()));
        fields.add_field_method_get("height", |_, this| Ok(this.0.height()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        add_drawing_methods!(methods);
        methods.add_method("new_texture", |_, this, (width, height): (i64, i64)| {
            this.0
                .new_texture(width, height)
                .map(LuaTexture)
                .map_err(mlua::Error::external)
        });
    }
}

impl UserData for LuaTexture {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.0.width()));
        fields.add_field_method_get("height", |_, this| Ok(this.0.height()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        add_drawing_methods!(methods);
        methods.add_method(
            "sprite",
            |_, this, (x, y, width, height): (i64, i64, i64, i64)| {
                this.0
                    .sprite(x, y, width, height)
                    .map(LuaSprite)
                    .map_err(mlua::Error::external)
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graphics::SharedScreen;
    use mlua::Lua;

    #[test]
    fn sprites_reference_atlas_clip_and_snapshot() {
        let lua = Lua::new();
        lua.globals()
            .set("graphics", LuaGraphics(Graphics::new(SharedScreen::new())))
            .unwrap();
        lua.load(r#"
            local atlas = graphics:new_texture(4, 2)
            atlas:set_pixel(1, 0, 0x7c00)
            atlas:set_pixel(2, 0, 0x03e0)
            atlas:set_pixel(1, 1, 0x001f)
            atlas:set_pixel(2, 1, 0x7fff)
            local sprite = atlas:sprite(1, 0, 2, 2)
            local target = graphics:new_texture(4, 3)
            for _, dest in ipairs({graphics, target}) do
                dest:clear()
                dest:draw_sprite(sprite, 1, 1)
                assert(dest:get_pixel(1, 1) == 0x7c00 and dest:get_pixel(2, 1) == 0x03e0)
                assert(dest:get_pixel(1, 2) == 0x001f and dest:get_pixel(2, 2) == 0x7fff)
                assert(dest:get_pixel(3, 1) == 0)
                dest:clear()
                dest:draw_sprite(sprite, -1, -1)
                assert(dest:get_pixel(0, 0) == 0x7fff and dest:get_pixel(1, 0) == 0)
                dest:draw_sprite(sprite, -1000000000, 1000000000)
                assert(dest:get_pixel(0, 0) == 0x7fff)
                assert(not pcall(function() dest:draw_sprite(atlas, 0, 0) end))
            end
            atlas:set_pixel(1, 0, 0x1234)
            graphics:draw_sprite(sprite, 0, 0)
            assert(graphics:get_pixel(0, 0) == 0x1234)
            for _, region in ipairs({
                {-1, 0, 1, 1}, {0, -1, 1, 1}, {0, 0, 0, 1}, {0, 0, 1, -1},
                {4, 0, 1, 1}, {0, 2, 1, 1}, {3, 0, 2, 1}, {0, 1, 1, 2},
                {1000000000, 0, 1000000000, 1}, {0, 1000000000, 1, 1000000000},
            }) do
                assert(not pcall(function() atlas:sprite(unpack(region)) end))
            end
            local overlap = graphics:new_texture(4, 1)
            overlap:set_pixel(0, 0, 1)
            overlap:set_pixel(1, 0, 2)
            overlap:set_pixel(2, 0, 3)
            local row = overlap:sprite(0, 0, 3, 1)
            overlap:draw_sprite(row, 1, 0)
            assert(overlap:get_pixel(1, 0) == 1 and overlap:get_pixel(2, 0) == 2 and overlap:get_pixel(3, 0) == 3)
        "#).exec().unwrap();
    }

    #[test]
    fn sprite_retains_allocation_until_collected() {
        let lua = Lua::new();
        lua.globals()
            .set("graphics", LuaGraphics(Graphics::new(SharedScreen::new())))
            .unwrap();
        lua.load(
            r#"
            local atlas = graphics:new_texture(320, 480)
            local bottom = graphics:new_texture(640, 240)
            atlas:set_pixel(0, 0, 0x1234)
            local sprite = atlas:sprite(0, 0, 1, 1)
            atlas = nil
            collectgarbage("collect")
            assert(not pcall(function() graphics:new_texture(1, 1) end))
            graphics:draw_sprite(sprite, 0, 0)
            assert(graphics:get_pixel(0, 0) == 0x1234 and bottom.width == 640)
            sprite = nil
            collectgarbage("collect")
            local replacement = graphics:new_texture(320, 480)
            assert(replacement:get_pixel(0, 0) == 0)
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn textured_primitives_on_both_targets() {
        let lua = Lua::new();
        lua.globals()
            .set("graphics", LuaGraphics(Graphics::new(SharedScreen::new())))
            .unwrap();
        lua.load(
            r#"
            local source = graphics:new_texture(2, 2)
            source:set_pixel(0, 0, 0x7c00)
            source:set_pixel(1, 0, 0x03e0)
            source:set_pixel(0, 1, 0x001f)
            source:set_pixel(1, 1, 0x7fff)
            local texture = graphics:new_texture(6, 6)
            for _, dest in ipairs({graphics, texture}) do
                dest:clear()
                dest:rect_texture(-1, -1, 4, 4, source, "stretch")
                assert(dest:get_pixel(0, 0) == 0x7c00)
                assert(dest:get_pixel(1, 0) == 0x03e0)
                assert(dest:get_pixel(0, 1) == 0x001f)
                assert(dest:get_pixel(1, 1) == 0x7fff)
                assert(dest:get_pixel(3, 0) == 0)
                dest:clear()
                dest:rect_texture(-1, -1, 4, 4, source, "repeat")
                assert(dest:get_pixel(0, 0) == 0x7fff)
                assert(dest:get_pixel(1, 0) == 0x001f)
                assert(dest:get_pixel(0, 1) == 0x03e0)
                assert(dest:get_pixel(1, 1) == 0x7c00)
                dest:clear()
                dest:circle_texture(1, 1, 1, source, "stretch")
                assert(dest:get_pixel(0, 0) == 0)
                assert(dest:get_pixel(1, 0) == 0x7c00)
                assert(dest:get_pixel(2, 1) == 0x03e0)
                assert(dest:get_pixel(1, 2) == 0x001f)
                dest:clear()
                dest:circle_texture(0, 0, 1, source, "repeat")
                assert(dest:get_pixel(0, 0) == 0x7fff)
                assert(dest:get_pixel(1, 0) == 0x001f)
                assert(dest:get_pixel(0, 1) == 0x03e0)
                assert(dest:get_pixel(1, 1) == 0)
                dest:circle_texture(3, 3, 0, source, "stretch")
                assert(dest:get_pixel(3, 3) == 0x7c00)
                dest:rect_texture(4, 4, 0, 2, source, "repeat")
                dest:circle_texture(4, 4, -1, source, "repeat")
                assert(dest:get_pixel(4, 4) == 0)
                dest:rect_texture(-1000000000, 0, 999999999, 1, source, "stretch")
                dest:circle_texture(-1000000000, -1000000000, 999999999, source, "repeat")
                assert(dest:get_pixel(4, 4) == 0)
                for _, invalid in ipairs({"Stretch", "", "repeat ", 1, false, {}}) do
                    assert(not pcall(function() dest:rect_texture(0, 0, 1, 1, source, invalid) end))
                    assert(not pcall(function() dest:circle_texture(0, 0, 1, source, invalid) end))
                end
                assert(not pcall(function() dest:rect_texture(0, 0, 1, 1, source) end))
                assert(not pcall(function() dest:circle_texture(0, 0, 1, source, nil) end))
                assert(not pcall(function() dest:rect_texture(0, 0, 1, 1, graphics, "repeat") end))
            end
            local overlap = graphics:new_texture(4, 1)
            overlap:set_pixel(0, 0, 0x7c00)
            overlap:set_pixel(1, 0, 0x03e0)
            overlap:set_pixel(2, 0, 0x001f)
            overlap:set_pixel(3, 0, 0x7fff)
            overlap:rect_texture(1, 0, 4, 1, overlap, "repeat")
            assert(overlap:get_pixel(1, 0) == 0x7c00)
            assert(overlap:get_pixel(2, 0) == 0x03e0)
            assert(overlap:get_pixel(3, 0) == 0x001f)
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn lua_textures_draw_locally_and_validate_inputs() {
        let lua = Lua::new();
        lua.globals()
            .set("graphics", LuaGraphics(Graphics::new(SharedScreen::new())))
            .unwrap();
        lua.load(
            r#"
            local t = graphics:new_texture(4, 4)
            local other = graphics:new_texture(4, 4)
            assert(t.width == 4 and t.height == 4)
            assert(not pcall(function() t.width = 5 end))
            assert(not pcall(function() t.height = 5 end))
            graphics:fill(0x7fff)
            t:fill(0x03e0)
            assert(t:get_pixel(3, 3) == 0x03e0)
            assert(other:get_pixel(0, 0) == 0)
            assert(graphics:get_pixel(0, 0) == 0x7fff)
            assert(t:get_pixel(-1, 0) == nil and t:get_pixel(4, 0) == nil)
            t:set_pixel(-1, 0, 0x7c00)
            t:set_pixel(1, 1, 0x7c00)
            assert(t:get_pixel(1, 1) == 0x7c00)
            assert(not pcall(function() t:set_pixel(-1, 0, 0x8000) end))
            assert(not pcall(function() t:fill(-1) end))
            assert(not pcall(function() t:rect(0, 0, 0, 0, -1) end))
            assert(not pcall(function() t:circle(0, 0, -1, 0x8000) end))
            assert(not pcall(function() t:line(0, 0, 0, 0, -1) end))
            t:clear()
            assert(t:get_pixel(1, 1) == 0)
            t:rect(-1, -1, 3, 3, 0x03e0)
            assert(t:get_pixel(1, 1) == 0x03e0 and t:get_pixel(2, 1) == 0)
            t:circle(3, 3, 1, 0x001f)
            assert(t:get_pixel(3, 3) == 0x001f and t:get_pixel(2, 3) == 0x001f)
            t:line(-2, 2, 2, 2, 0x7c00)
            assert(t:get_pixel(0, 2) == 0x7c00 and t:get_pixel(2, 2) == 0x7c00)
            assert(other:get_pixel(0, 0) == 0 and graphics:get_pixel(0, 0) == 0x7fff)
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn lua_texture_allocation_errors_and_reuses_space() {
        let lua = Lua::new();
        lua.globals()
            .set("graphics", LuaGraphics(Graphics::new(SharedScreen::new())))
            .unwrap();
        lua.load(
            r#"
            assert(not pcall(function() graphics:new_texture(0, 1) end))
            assert(not pcall(function() graphics:new_texture(1, -1) end))
            assert(not pcall(function() graphics:new_texture(math.maxinteger, 1) end))
            local right = graphics:new_texture(320, 480)
            local bottom = graphics:new_texture(640, 240)
            assert(not pcall(function() graphics:new_texture(1, 1) end))
            right = nil
            collectgarbage("collect")
            local replacement = graphics:new_texture(320, 480)
            assert(replacement:get_pixel(0, 0) == 0 and bottom.width == 640)
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn lua_graphics_clip_shapes_and_reject_invalid_colors() {
        let lua = Lua::new();
        lua.globals()
            .set("graphics", LuaGraphics(Graphics::new(SharedScreen::new())))
            .unwrap();
        lua.load(
            r#"
            assert(graphics.width > 3 and graphics.height > 3)
            graphics:fill(0x7fff)
            assert(graphics:get_pixel(0, 0) == 0x7fff)
            graphics:clear()
            assert(graphics:get_pixel(-1, 0) == nil)
            assert(graphics:get_pixel(graphics.width, 0) == nil)
            graphics:set_pixel(-1, 0, 0x7c00)
            graphics:set_pixel(0, 0, 0x7c00)
            assert(graphics:get_pixel(0, 0) == 0x7c00)
            assert(not pcall(function() graphics:fill(-1) end))
            assert(not pcall(function() graphics:set_pixel(-1, 0, 0x8000) end))
            assert(not pcall(function() graphics:circle(0, 0, -1, 0x8000) end))

            graphics:clear()
            graphics:rect(-1, -1, 3, 3, 0x03e0)
            assert(graphics:get_pixel(0, 0) == 0x03e0)
            assert(graphics:get_pixel(1, 1) == 0x03e0)
            assert(graphics:get_pixel(2, 1) == 0)
            graphics:rect(2, 2, 0, 2, 0x7fff)
            graphics:rect(2, 2, 2, -1, 0x7fff)
            assert(graphics:get_pixel(2, 2) == 0)
            graphics:rect(graphics.width - 1, graphics.height - 1, 2, 2, 0x001f)
            assert(graphics:get_pixel(graphics.width - 1, graphics.height - 1) == 0x001f)

            graphics:clear()
            graphics:circle(1, 1, 0, 0x001f)
            assert(graphics:get_pixel(1, 1) == 0x001f)
            assert(graphics:get_pixel(1, 0) == 0)
            graphics:circle(0, 0, 2, 0x7c00)
            assert(graphics:get_pixel(2, 0) == 0x7c00)
            assert(graphics:get_pixel(2, 2) == 0)
            graphics:circle(3, 3, -1, 0x7fff)
            assert(graphics:get_pixel(3, 3) == 0)

            graphics:clear()
            graphics:line(-2, 1, 2, 1, 0x7fff)
            assert(graphics:get_pixel(0, 1) == 0x7fff)
            assert(graphics:get_pixel(2, 1) == 0x7fff)
            assert(graphics:get_pixel(3, 1) == 0)
            graphics:line(3, 3, 1, 1, 0x03e0)
            assert(graphics:get_pixel(1, 1) == 0x03e0)
            assert(graphics:get_pixel(2, 2) == 0x03e0)
            assert(graphics:get_pixel(3, 3) == 0x03e0)
            graphics:line(0, -1000000000, 0, 1000000000, 0x001f)
            assert(graphics:get_pixel(0, graphics.height - 1) == 0x001f)
            graphics:circle(-1000000000, 0, 999999999, 0x7fff)
            graphics:rect(-1000000000, 0, 999999999, 1, 0x7fff)
        "#,
        )
        .exec()
        .unwrap();
    }
}
