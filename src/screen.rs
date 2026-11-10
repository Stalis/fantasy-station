use std::cell::RefCell;
use std::rc::Rc;

use mlua::{UserData, UserDataFields, UserDataMethods};
use sdl3::pixels::Color;

use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};

const BUFFER_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

fn unpack_color(value: i64) -> mlua::Result<Color> {
    if !(0..=0x7fff).contains(&value) {
        return Err(mlua::Error::external(
            "color must be in the range 0..0x7fff",
        ));
    }
    let expand = |channel: i64| ((channel << 3) | (channel >> 2)) as u8;
    Ok(Color::RGBA(
        expand((value >> 10) & 31),
        expand((value >> 5) & 31),
        expand(value & 31),
        255,
    ))
}

fn pack_color(color: Color) -> u16 {
    (u16::from(color.r >> 3) << 10) | (u16::from(color.g >> 3) << 5) | u16::from(color.b >> 3)
}

struct ScreenCoordinate {
    x: usize,
    y: usize,
}

impl ScreenCoordinate {
    fn to_index(&self) -> usize {
        (self.y * SCREEN_WIDTH) + self.x
    }
}

pub(crate) struct Screen {
    pub(crate) buffer: Box<[Color]>,
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

    pub(crate) fn get_pixel(&self, x: usize, y: usize) -> Option<Color> {
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

    fn rect(&mut self, x: i64, y: i64, width: i64, height: i64, color: Color) {
        if width <= 0 || height <= 0 {
            return;
        }
        let left = i128::from(x).clamp(0, SCREEN_WIDTH as i128) as usize;
        let top = i128::from(y).clamp(0, SCREEN_HEIGHT as i128) as usize;
        let right = (i128::from(x) + i128::from(width)).clamp(0, SCREEN_WIDTH as i128) as usize;
        let bottom = (i128::from(y) + i128::from(height)).clamp(0, SCREEN_HEIGHT as i128) as usize;
        for row in top..bottom {
            self.buffer[row * SCREEN_WIDTH + left..row * SCREEN_WIDTH + right].fill(color);
        }
    }

    fn circle(&mut self, cx: i64, cy: i64, radius: i64, color: Color) {
        if radius < 0 {
            return;
        }
        let left = (i128::from(cx) - i128::from(radius)).clamp(0, SCREEN_WIDTH as i128) as usize;
        let right =
            (i128::from(cx) + i128::from(radius) + 1).clamp(0, SCREEN_WIDTH as i128) as usize;
        let top = (i128::from(cy) - i128::from(radius)).clamp(0, SCREEN_HEIGHT as i128) as usize;
        let bottom =
            (i128::from(cy) + i128::from(radius) + 1).clamp(0, SCREEN_HEIGHT as i128) as usize;
        let radius = radius as u128;
        let radius_squared = radius * radius;
        for y in top..bottom {
            let dy = (y as i128 - i128::from(cy)).unsigned_abs();
            for x in left..right {
                let dx = (x as i128 - i128::from(cx)).unsigned_abs();
                if dx <= radius && dx * dx <= radius_squared - dy * dy {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: Color) {
        let (mut x0, mut y0, mut x1, mut y1) = (
            i128::from(x0),
            i128::from(y0),
            i128::from(x1),
            i128::from(y1),
        );
        let steep = (y1 - y0).abs() > (x1 - x0).abs();
        if steep {
            std::mem::swap(&mut x0, &mut y0);
            std::mem::swap(&mut x1, &mut y1);
        }
        if x0 > x1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
        }
        let major = (x1 - x0) as u128;
        let minor = (y1 - y0).unsigned_abs();
        let limit = if steep { SCREEN_HEIGHT } else { SCREEN_WIDTH } as i128;
        let start = x0.max(0);
        let end = x1.min(limit - 1);
        for x in start..=end {
            let offset = if major == 0 {
                0
            } else {
                let product = (x - x0) as u128 * minor;
                (product / major + u128::from(product % major >= (major + 1) / 2)) as i128
            };
            let y = if y1 >= y0 { y0 + offset } else { y0 - offset };
            let (px, py) = if steep { (y, x) } else { (x, y) };
            if px >= 0 && px < SCREEN_WIDTH as i128 && py >= 0 && py < SCREEN_HEIGHT as i128 {
                self.set_pixel(px as usize, py as usize, color);
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct SharedScreen(pub(crate) Rc<RefCell<Screen>>);

impl SharedScreen {
    pub(crate) fn new() -> Self {
        Self(Rc::new(RefCell::new(Screen::new())))
    }
}

impl UserData for SharedScreen {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| Ok(this.0.borrow().width()));
        fields.add_field_method_get("height", |_, this| Ok(this.0.borrow().height()));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("set_pixel", |_, this, (x, y, color): (i64, i64, i64)| {
            let color = unpack_color(color)?;
            if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
                this.0.borrow_mut().set_pixel(x, y, color);
            }
            Ok(())
        });

        methods.add_method("get_pixel", |_, this, (x, y): (i64, i64)| {
            Ok(usize::try_from(x)
                .ok()
                .zip(usize::try_from(y).ok())
                .and_then(|(x, y)| this.0.borrow().get_pixel(x, y))
                .map(pack_color))
        });

        methods.add_method("fill", |_, this, color: i64| {
            this.0.borrow_mut().fill(unpack_color(color)?);
            Ok(())
        });

        methods.add_method("clear", |_, this, ()| {
            this.0.borrow_mut().clear();
            Ok(())
        });

        methods.add_method(
            "rect",
            |_, this, (x, y, width, height, color): (i64, i64, i64, i64, i64)| {
                this.0
                    .borrow_mut()
                    .rect(x, y, width, height, unpack_color(color)?);
                Ok(())
            },
        );
        methods.add_method(
            "circle",
            |_, this, (x, y, radius, color): (i64, i64, i64, i64)| {
                this.0
                    .borrow_mut()
                    .circle(x, y, radius, unpack_color(color)?);
                Ok(())
            },
        );
        methods.add_method(
            "line",
            |_, this, (x0, y0, x1, y1, color): (i64, i64, i64, i64, i64)| {
                this.0
                    .borrow_mut()
                    .line(x0, y0, x1, y1, unpack_color(color)?);
                Ok(())
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn packed_colors_round_trip_and_are_opaque() {
        for (packed, expected) in [
            (0x7c00, Color::RGBA(255, 0, 0, 255)),
            (0x03e0, Color::RGBA(0, 255, 0, 255)),
            (0x001f, Color::RGBA(0, 0, 255, 255)),
        ] {
            assert_eq!(unpack_color(packed).unwrap(), expected);
        }
        for packed in 0..=0x7fff {
            assert_eq!(i64::from(pack_color(unpack_color(packed).unwrap())), packed);
        }
    }

    #[test]
    fn lua_graphics_clip_shapes_and_reject_invalid_colors() {
        let lua = Lua::new();
        lua.globals().set("graphics", SharedScreen::new()).unwrap();
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

    #[test]
    fn extreme_coordinates_are_clipped_without_overflow() {
        let mut screen = Screen::new();
        screen.line(i64::MIN, i64::MIN, i64::MAX, i64::MAX, Color::WHITE);
        assert_eq!(screen.get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(screen.get_pixel(1, 1), Some(Color::WHITE));
        screen.circle(i64::MIN, i64::MIN, i64::MAX, Color::WHITE);
        screen.rect(i64::MIN, i64::MIN, i64::MAX, i64::MAX, Color::WHITE);
    }
}
