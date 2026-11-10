use std::rc::Rc;

use sdl3::pixels::Color;

use crate::core::graphics::{Allocation, Mapping, Region, SharedScreen};
use crate::core::input::{Button, SharedInput};

#[derive(Clone)]
pub(crate) struct Graphics(SharedScreen);

fn unpack_color(value: i64) -> Result<Color, &'static str> {
    if !(0..=0x7fff).contains(&value) {
        return Err("color must be in the range 0..0x7fff");
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

fn mapping(value: &str) -> Result<Mapping, &'static str> {
    match value {
        "stretch" => Ok(Mapping::Stretch),
        "repeat" => Ok(Mapping::Repeat),
        _ => Err("mapping must be 'stretch' or 'repeat'"),
    }
}

impl Graphics {
    pub(crate) fn new(screen: SharedScreen) -> Self {
        Self(screen)
    }
    pub(crate) fn width(&self) -> usize {
        self.0.0.borrow().width()
    }
    pub(crate) fn height(&self) -> usize {
        self.0.0.borrow().height()
    }
    pub(crate) fn new_texture(&self, width: i64, height: i64) -> Result<Texture, &'static str> {
        let allocation = self.0.0.borrow_mut().allocate(width, height)?;
        Ok(Texture {
            screen: self.0.clone(),
            allocation,
        })
    }
    pub(crate) fn set_pixel(&self, x: i64, y: i64, color: i64) -> Result<(), &'static str> {
        let color = unpack_color(color)?;
        if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
            self.0.0.borrow_mut().set_pixel(x, y, color);
        }
        Ok(())
    }
    pub(crate) fn get_pixel(&self, x: i64, y: i64) -> Option<u16> {
        usize::try_from(x)
            .ok()
            .zip(usize::try_from(y).ok())
            .and_then(|(x, y)| self.0.0.borrow().get_pixel(x, y))
            .map(pack_color)
    }
    pub(crate) fn fill(&self, color: i64) -> Result<(), &'static str> {
        self.0.0.borrow_mut().fill(unpack_color(color)?);
        Ok(())
    }
    pub(crate) fn clear(&self) {
        self.0.0.borrow_mut().clear();
    }
    pub(crate) fn rect(
        &self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        color: i64,
    ) -> Result<(), &'static str> {
        self.0
            .0
            .borrow_mut()
            .rect(x, y, width, height, unpack_color(color)?);
        Ok(())
    }
    pub(crate) fn circle(
        &self,
        x: i64,
        y: i64,
        radius: i64,
        color: i64,
    ) -> Result<(), &'static str> {
        self.0
            .0
            .borrow_mut()
            .circle(x, y, radius, unpack_color(color)?);
        Ok(())
    }
    pub(crate) fn rect_texture(
        &self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        source: &Texture,
        mode: &str,
    ) -> Result<(), &'static str> {
        let mapping = mapping(mode)?;
        if !Rc::ptr_eq(&self.0.0, &source.screen.0) {
            return Err("texture belongs to a different screen");
        }
        let mut screen = self.0.0.borrow_mut();
        let dest = screen.screen_region();
        screen.rect_texture_in(dest, x, y, width, height, source.allocation.region, mapping);
        Ok(())
    }
    pub(crate) fn circle_texture(
        &self,
        x: i64,
        y: i64,
        radius: i64,
        source: &Texture,
        mode: &str,
    ) -> Result<(), &'static str> {
        let mapping = mapping(mode)?;
        if !Rc::ptr_eq(&self.0.0, &source.screen.0) {
            return Err("texture belongs to a different screen");
        }
        let mut screen = self.0.0.borrow_mut();
        let dest = screen.screen_region();
        screen.circle_texture_in(dest, x, y, radius, source.allocation.region, mapping);
        Ok(())
    }
    pub(crate) fn draw_sprite(&self, sprite: &Sprite, x: i64, y: i64) -> Result<(), &'static str> {
        if !Rc::ptr_eq(&self.0.0, &sprite.texture.screen.0) {
            return Err("sprite belongs to a different screen");
        }
        let mut screen = self.0.0.borrow_mut();
        let dest = screen.screen_region();
        screen.blit_in(dest, sprite.region, x, y);
        Ok(())
    }
    pub(crate) fn line(
        &self,
        x0: i64,
        y0: i64,
        x1: i64,
        y1: i64,
        color: i64,
    ) -> Result<(), &'static str> {
        self.0
            .0
            .borrow_mut()
            .line(x0, y0, x1, y1, unpack_color(color)?);
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Texture {
    screen: SharedScreen,
    allocation: Rc<Allocation>,
}

pub(crate) struct Sprite {
    texture: Texture,
    region: Region,
}

impl Texture {
    pub(crate) fn width(&self) -> usize {
        self.allocation.region.width
    }
    pub(crate) fn height(&self) -> usize {
        self.allocation.region.height
    }
    pub(crate) fn sprite(
        &self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
    ) -> Result<Sprite, &'static str> {
        let invalid = "sprite region must be positive and within the texture";
        let (x, y, width, height) = (
            usize::try_from(x).map_err(|_| invalid)?,
            usize::try_from(y).map_err(|_| invalid)?,
            usize::try_from(width).map_err(|_| invalid)?,
            usize::try_from(height).map_err(|_| invalid)?,
        );
        if width == 0
            || height == 0
            || x.checked_add(width)
                .filter(|&end| end <= self.width())
                .is_none()
            || y.checked_add(height)
                .filter(|&end| end <= self.height())
                .is_none()
        {
            return Err(invalid);
        }
        let atlas = self.allocation.region;
        let region = Region {
            x: atlas.x.checked_add(x).ok_or(invalid)?,
            y: atlas.y.checked_add(y).ok_or(invalid)?,
            width,
            height,
        };
        Ok(Sprite {
            texture: self.clone(),
            region,
        })
    }
    pub(crate) fn draw_sprite(&self, sprite: &Sprite, x: i64, y: i64) -> Result<(), &'static str> {
        if !Rc::ptr_eq(&self.screen.0, &sprite.texture.screen.0) {
            return Err("sprite belongs to a different screen");
        }
        self.screen
            .0
            .borrow_mut()
            .blit_in(self.allocation.region, sprite.region, x, y);
        Ok(())
    }
    pub(crate) fn set_pixel(&self, x: i64, y: i64, color: i64) -> Result<(), &'static str> {
        let color = unpack_color(color)?;
        if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
            self.screen
                .0
                .borrow_mut()
                .set_pixel_in(self.allocation.region, x, y, color);
        }
        Ok(())
    }
    pub(crate) fn get_pixel(&self, x: i64, y: i64) -> Option<u16> {
        usize::try_from(x)
            .ok()
            .zip(usize::try_from(y).ok())
            .and_then(|(x, y)| {
                self.screen
                    .0
                    .borrow()
                    .get_pixel_in(self.allocation.region, x, y)
            })
            .map(pack_color)
    }
    pub(crate) fn fill(&self, color: i64) -> Result<(), &'static str> {
        self.screen
            .0
            .borrow_mut()
            .fill_in(self.allocation.region, unpack_color(color)?);
        Ok(())
    }
    pub(crate) fn clear(&self) {
        self.screen
            .0
            .borrow_mut()
            .fill_in(self.allocation.region, Color::RGBA(0, 0, 0, 255));
    }
    pub(crate) fn rect(
        &self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        color: i64,
    ) -> Result<(), &'static str> {
        self.screen.0.borrow_mut().rect_in(
            self.allocation.region,
            x,
            y,
            width,
            height,
            unpack_color(color)?,
        );
        Ok(())
    }
    pub(crate) fn circle(
        &self,
        x: i64,
        y: i64,
        radius: i64,
        color: i64,
    ) -> Result<(), &'static str> {
        self.screen.0.borrow_mut().circle_in(
            self.allocation.region,
            x,
            y,
            radius,
            unpack_color(color)?,
        );
        Ok(())
    }
    pub(crate) fn rect_texture(
        &self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        source: &Texture,
        mode: &str,
    ) -> Result<(), &'static str> {
        let mapping = mapping(mode)?;
        if !Rc::ptr_eq(&self.screen.0, &source.screen.0) {
            return Err("texture belongs to a different screen");
        }
        self.screen.0.borrow_mut().rect_texture_in(
            self.allocation.region,
            x,
            y,
            width,
            height,
            source.allocation.region,
            mapping,
        );
        Ok(())
    }
    pub(crate) fn circle_texture(
        &self,
        x: i64,
        y: i64,
        radius: i64,
        source: &Texture,
        mode: &str,
    ) -> Result<(), &'static str> {
        let mapping = mapping(mode)?;
        if !Rc::ptr_eq(&self.screen.0, &source.screen.0) {
            return Err("texture belongs to a different screen");
        }
        self.screen.0.borrow_mut().circle_texture_in(
            self.allocation.region,
            x,
            y,
            radius,
            source.allocation.region,
            mapping,
        );
        Ok(())
    }
    pub(crate) fn line(
        &self,
        x0: i64,
        y0: i64,
        x1: i64,
        y1: i64,
        color: i64,
    ) -> Result<(), &'static str> {
        self.screen.0.borrow_mut().line_in(
            self.allocation.region,
            x0,
            y0,
            x1,
            y1,
            unpack_color(color)?,
        );
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Input(SharedInput);

impl Input {
    pub(crate) fn new(input: SharedInput) -> Self {
        Self(input)
    }
    pub(crate) fn down(&self, slot: usize, name: &str) -> Option<bool> {
        Some(self.0.0.borrow()[slot].down(Button::parse(name)?))
    }
    pub(crate) fn pressed(&self, slot: usize, name: &str) -> Option<bool> {
        Some(self.0.0.borrow()[slot].pressed(Button::parse(name)?))
    }
    pub(crate) fn released(&self, slot: usize, name: &str) -> Option<bool> {
        Some(self.0.0.borrow()[slot].released(Button::parse(name)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textured_primitives_reject_sources_from_another_screen() {
        let first = Graphics::new(SharedScreen::new());
        let second = Graphics::new(SharedScreen::new());
        let source = first.new_texture(2, 2).unwrap();
        let dest = second.new_texture(2, 2).unwrap();
        assert!(second.rect_texture(0, 0, 2, 2, &source, "repeat").is_err());
        assert!(second.circle_texture(0, 0, 1, &source, "stretch").is_err());
        assert!(dest.rect_texture(0, 0, 2, 2, &source, "repeat").is_err());
        assert!(dest.circle_texture(0, 0, 1, &source, "stretch").is_err());
        let sprite = source.sprite(0, 0, 1, 1).unwrap();
        assert!(second.draw_sprite(&sprite, 0, 0).is_err());
        assert!(dest.draw_sprite(&sprite, 0, 0).is_err());
    }

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
}
