use std::cell::RefCell;
use std::rc::{Rc, Weak};

use sdl3::pixels::Color;

use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};

const VRAM_WIDTH: usize = SCREEN_WIDTH * 3;
const VRAM_HEIGHT: usize = SCREEN_HEIGHT * 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Copy)]
pub(crate) enum Mapping {
    Stretch,
    Repeat,
}

#[derive(Debug)]
pub(crate) struct Allocation {
    pub region: Region,
}

fn pack_color(color: Color) -> u16 {
    (u16::from(color.r >> 3) << 10) | (u16::from(color.g >> 3) << 5) | u16::from(color.b >> 3)
}

fn unpack_color(pixel: u16) -> Color {
    let expand = |channel: u16| ((channel << 3) | (channel >> 2)) as u8;
    Color::RGBA(
        expand((pixel >> 10) & 31),
        expand((pixel >> 5) & 31),
        expand(pixel & 31),
        255,
    )
}

pub(crate) struct Screen {
    vram: Box<[u16]>,
    drawing: usize,
    displayed: usize,
    allocations: Vec<Weak<Allocation>>,
}

impl Screen {
    fn new() -> Self {
        Screen {
            vram: vec![0; VRAM_WIDTH * VRAM_HEIGHT].into_boxed_slice(),
            drawing: 0,
            displayed: 0,
            allocations: Vec::new(),
        }
    }

    pub(crate) fn allocate(
        &mut self,
        width: i64,
        height: i64,
    ) -> Result<Rc<Allocation>, &'static str> {
        let width = usize::try_from(width).map_err(|_| "invalid allocation dimensions")?;
        let height = usize::try_from(height).map_err(|_| "invalid allocation dimensions")?;
        if width == 0 || height == 0 || width > VRAM_WIDTH || height > VRAM_HEIGHT {
            return Err("invalid allocation dimensions");
        }

        let mut occupied = vec![Region {
            x: 0,
            y: 0,
            width: SCREEN_WIDTH * 2,
            height: SCREEN_HEIGHT,
        }];
        self.allocations.retain(|weak| {
            if let Some(allocation) = weak.upgrade() {
                occupied.push(allocation.region);
                true
            } else {
                false
            }
        });

        for y in 0..=VRAM_HEIGHT - height {
            let mut x = 0;
            while x <= VRAM_WIDTH - width {
                if let Some(region) = occupied.iter().find(|region| {
                    x < region.x + region.width
                        && region.x < x + width
                        && y < region.y + region.height
                        && region.y < y + height
                }) {
                    x = region.x + region.width;
                } else {
                    for row in y..y + height {
                        let start = row * VRAM_WIDTH + x;
                        self.vram[start..start + width].fill(0);
                    }
                    let allocation = Rc::new(Allocation {
                        region: Region {
                            x,
                            y,
                            width,
                            height,
                        },
                    });
                    self.allocations.push(Rc::downgrade(&allocation));
                    return Ok(allocation);
                }
            }
        }
        Err("not enough VRAM")
    }

    pub(crate) fn begin_frame(&mut self) {
        let next = 1 - self.drawing;
        for y in 0..SCREEN_HEIGHT {
            let source = y * VRAM_WIDTH + self.drawing * SCREEN_WIDTH;
            let destination = y * VRAM_WIDTH + next * SCREEN_WIDTH;
            self.vram
                .copy_within(source..source + SCREEN_WIDTH, destination);
        }
        self.drawing = next;
    }

    pub(crate) fn publish_frame(&mut self) {
        self.displayed = self.drawing;
    }

    pub(crate) fn displayed_rows(&self) -> impl Iterator<Item = &[u16]> {
        let x = self.displayed * SCREEN_WIDTH;
        self.vram
            .chunks_exact(VRAM_WIDTH)
            .take(SCREEN_HEIGHT)
            .map(move |row| &row[x..x + SCREEN_WIDTH])
    }

    pub(crate) fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    pub(crate) fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    pub(crate) fn screen_region(&self) -> Region {
        Region {
            x: self.drawing * SCREEN_WIDTH,
            y: 0,
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
        }
    }

    pub(crate) fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        self.set_pixel_in(self.screen_region(), x, y, color);
    }

    pub(crate) fn set_pixel_in(&mut self, region: Region, x: usize, y: usize, color: Color) {
        if x < region.width && y < region.height {
            self.vram[(region.y + y) * VRAM_WIDTH + region.x + x] = pack_color(color);
        }
    }

    pub(crate) fn get_pixel(&self, x: usize, y: usize) -> Option<Color> {
        self.get_pixel_in(self.screen_region(), x, y)
    }

    pub(crate) fn get_pixel_in(&self, region: Region, x: usize, y: usize) -> Option<Color> {
        if x < region.width && y < region.height {
            Some(unpack_color(
                self.vram[(region.y + y) * VRAM_WIDTH + region.x + x],
            ))
        } else {
            None
        }
    }

    pub(crate) fn fill(&mut self, color: Color) {
        self.fill_in(self.screen_region(), color);
    }

    pub(crate) fn fill_in(&mut self, region: Region, color: Color) {
        let color = pack_color(color);
        for y in region.y..region.y + region.height {
            let start = y * VRAM_WIDTH + region.x;
            self.vram[start..start + region.width].fill(color);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.fill(Color::BLACK);
    }

    pub(crate) fn rect(&mut self, x: i64, y: i64, width: i64, height: i64, color: Color) {
        self.rect_in(self.screen_region(), x, y, width, height, color);
    }

    pub(crate) fn rect_in(
        &mut self,
        region: Region,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        color: Color,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        let left = i128::from(x).clamp(0, region.width as i128) as usize;
        let top = i128::from(y).clamp(0, region.height as i128) as usize;
        let right = (i128::from(x) + i128::from(width)).clamp(0, region.width as i128) as usize;
        let bottom = (i128::from(y) + i128::from(height)).clamp(0, region.height as i128) as usize;
        let color = pack_color(color);
        for row in top..bottom {
            let start = (region.y + row) * VRAM_WIDTH + region.x;
            self.vram[start + left..start + right].fill(color);
        }
    }

    pub(crate) fn circle(&mut self, cx: i64, cy: i64, radius: i64, color: Color) {
        self.circle_in(self.screen_region(), cx, cy, radius, color);
    }

    pub(crate) fn circle_in(
        &mut self,
        region: Region,
        cx: i64,
        cy: i64,
        radius: i64,
        color: Color,
    ) {
        if radius < 0 {
            return;
        }
        let left = (i128::from(cx) - i128::from(radius)).clamp(0, region.width as i128) as usize;
        let right =
            (i128::from(cx) + i128::from(radius) + 1).clamp(0, region.width as i128) as usize;
        let top = (i128::from(cy) - i128::from(radius)).clamp(0, region.height as i128) as usize;
        let bottom =
            (i128::from(cy) + i128::from(radius) + 1).clamp(0, region.height as i128) as usize;
        let radius = radius as u128;
        let radius_squared = radius * radius;
        for y in top..bottom {
            let dy = (y as i128 - i128::from(cy)).unsigned_abs();
            for x in left..right {
                let dx = (x as i128 - i128::from(cx)).unsigned_abs();
                if dx <= radius && dx * dx <= radius_squared - dy * dy {
                    self.set_pixel_in(region, x, y, color);
                }
            }
        }
    }

    pub(crate) fn blit_in(&mut self, dest: Region, src: Region, x: i64, y: i64) {
        self.texture_in(
            dest,
            i128::from(x),
            i128::from(y),
            src.width as i128,
            src.height as i128,
            src,
            Mapping::Stretch,
            None,
        );
    }

    pub(crate) fn rect_texture_in(
        &mut self,
        dest: Region,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
        src: Region,
        mapping: Mapping,
    ) {
        if width <= 0 || height <= 0 {
            return;
        }
        self.texture_in(
            dest,
            i128::from(x),
            i128::from(y),
            i128::from(width),
            i128::from(height),
            src,
            mapping,
            None,
        );
    }

    pub(crate) fn circle_texture_in(
        &mut self,
        dest: Region,
        cx: i64,
        cy: i64,
        radius: i64,
        src: Region,
        mapping: Mapping,
    ) {
        if radius < 0 {
            return;
        }
        let radius = i128::from(radius);
        let size = radius * 2 + 1;
        self.texture_in(
            dest,
            i128::from(cx) - radius,
            i128::from(cy) - radius,
            size,
            size,
            src,
            mapping,
            Some(radius as u128),
        );
    }

    fn texture_in(
        &mut self,
        dest: Region,
        x: i128,
        y: i128,
        width: i128,
        height: i128,
        src: Region,
        mapping: Mapping,
        radius: Option<u128>,
    ) {
        let left = x.clamp(0, dest.width as i128) as usize;
        let top = y.clamp(0, dest.height as i128) as usize;
        let right = (x + width).clamp(0, dest.width as i128) as usize;
        let bottom = (y + height).clamp(0, dest.height as i128) as usize;
        if left >= right || top >= bottom {
            return;
        }
        let overlap = dest.x < src.x + src.width
            && src.x < dest.x + dest.width
            && dest.y < src.y + src.height
            && src.y < dest.y + dest.height;
        let snapshot = overlap.then(|| {
            let mut pixels = Vec::with_capacity(src.width * src.height);
            for row in src.y..src.y + src.height {
                let start = row * VRAM_WIDTH + src.x;
                pixels.extend_from_slice(&self.vram[start..start + src.width]);
            }
            pixels
        });
        for row in top..bottom {
            let local_y = row as i128 - y;
            for col in left..right {
                let local_x = col as i128 - x;
                if let Some(radius) = radius {
                    let dx = (local_x - radius as i128).unsigned_abs();
                    let dy = (local_y - radius as i128).unsigned_abs();
                    if dx > radius || dy > radius || dx * dx > radius * radius - dy * dy {
                        continue;
                    }
                }
                let (sx, sy) = match mapping {
                    Mapping::Stretch => (
                        (local_x * src.width as i128 / width) as usize,
                        (local_y * src.height as i128 / height) as usize,
                    ),
                    Mapping::Repeat => (
                        (local_x % src.width as i128) as usize,
                        (local_y % src.height as i128) as usize,
                    ),
                };
                let pixel = match &snapshot {
                    Some(pixels) => pixels[sy * src.width + sx],
                    None => self.vram[(src.y + sy) * VRAM_WIDTH + src.x + sx],
                };
                self.vram[(dest.y + row) * VRAM_WIDTH + dest.x + col] = pixel;
            }
        }
    }

    pub(crate) fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: Color) {
        self.line_in(self.screen_region(), x0, y0, x1, y1, color);
    }

    pub(crate) fn line_in(
        &mut self,
        region: Region,
        x0: i64,
        y0: i64,
        x1: i64,
        y1: i64,
        color: Color,
    ) {
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
        let limit = if steep { region.height } else { region.width } as i128;
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
            if px >= 0 && px < region.width as i128 && py >= 0 && py < region.height as i128 {
                self.set_pixel_in(region, px as usize, py as usize, color);
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

#[cfg(test)]
mod allocation_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textured_shapes_clip_map_and_snapshot() {
        let mut screen = Screen::new();
        let source = screen.allocate(2, 2).unwrap();
        let dest = screen.allocate(5, 4).unwrap();
        let src = source.region;
        let dst = dest.region;
        screen.set_pixel_in(src, 0, 0, Color::RED);
        screen.set_pixel_in(src, 1, 0, Color::GREEN);
        screen.set_pixel_in(src, 0, 1, Color::BLUE);
        screen.set_pixel_in(src, 1, 1, Color::WHITE);
        screen.rect_texture_in(dst, -1, 0, 4, 2, src, Mapping::Stretch);
        assert_eq!(screen.get_pixel_in(dst, 0, 0), Some(Color::RED));
        assert_eq!(screen.get_pixel_in(dst, 1, 0), Some(Color::GREEN));
        assert_eq!(screen.get_pixel_in(dst, 0, 1), Some(Color::BLUE));
        screen.rect_texture_in(dst, -1, 2, 5, 1, src, Mapping::Repeat);
        assert_eq!(screen.get_pixel_in(dst, 0, 2), Some(Color::GREEN));
        assert_eq!(screen.get_pixel_in(dst, 1, 2), Some(Color::RED));
        screen.fill_in(dst, Color::BLACK);
        screen.circle_texture_in(dst, 1, 1, 1, src, Mapping::Repeat);
        assert_eq!(screen.get_pixel_in(dst, 0, 0), Some(Color::BLACK));
        assert_eq!(screen.get_pixel_in(dst, 1, 0), Some(Color::GREEN));
        assert_eq!(screen.get_pixel_in(dst, 1, 1), Some(Color::WHITE));
        screen.rect_texture_in(src, 1, 0, 2, 2, src, Mapping::Repeat);
        assert_eq!(screen.get_pixel_in(src, 1, 0), Some(Color::RED));
        assert_eq!(screen.get_pixel_in(src, 1, 1), Some(Color::BLUE));
        screen.circle_texture_in(dst, i64::MIN, i64::MIN, i64::MAX, src, Mapping::Stretch);
        screen.circle_texture_in(dst, 0, 0, i64::MAX, src, Mapping::Stretch);
        screen.rect_texture_in(dst, i64::MIN, 0, i64::MAX, 1, src, Mapping::Repeat);
        screen.circle_texture_in(dst, 4, 3, 0, src, Mapping::Stretch);
        assert_eq!(screen.get_pixel_in(dst, 4, 3), Some(Color::RED));
    }

    #[test]
    fn blit_subregion_clips_and_reads_current_pixels() {
        let mut screen = Screen::new();
        let atlas = screen.allocate(4, 3).unwrap();
        let target = screen.allocate(3, 2).unwrap();
        let src = Region {
            x: atlas.region.x + 1,
            y: atlas.region.y + 1,
            width: 2,
            height: 2,
        };
        screen.set_pixel_in(src, 0, 0, Color::RED);
        screen.set_pixel_in(src, 1, 0, Color::GREEN);
        screen.set_pixel_in(src, 0, 1, Color::BLUE);
        screen.set_pixel_in(src, 1, 1, Color::WHITE);
        screen.blit_in(target.region, src, -1, 0);
        assert_eq!(screen.get_pixel_in(target.region, 0, 0), Some(Color::GREEN));
        assert_eq!(screen.get_pixel_in(target.region, 0, 1), Some(Color::WHITE));
        assert_eq!(screen.get_pixel_in(target.region, 1, 0), Some(Color::BLACK));
        screen.set_pixel_in(src, 1, 0, Color::BLUE);
        screen.blit_in(target.region, src, 1, 0);
        assert_eq!(screen.get_pixel_in(target.region, 1, 0), Some(Color::RED));
        assert_eq!(screen.get_pixel_in(target.region, 2, 0), Some(Color::BLUE));
        screen.blit_in(target.region, src, i64::MIN, i64::MAX);
        screen.blit_in(target.region, src, i64::MAX, i64::MIN);
        assert_eq!(screen.get_pixel_in(target.region, 1, 0), Some(Color::RED));
        assert_eq!(screen.get_pixel_in(target.region, 2, 0), Some(Color::BLUE));
    }

    #[test]
    fn blit_overlapping_subregion_uses_snapshot() {
        let mut screen = Screen::new();
        let allocation = screen.allocate(4, 1).unwrap();
        let dest = allocation.region;
        screen.set_pixel_in(dest, 0, 0, Color::RED);
        screen.set_pixel_in(dest, 1, 0, Color::GREEN);
        screen.set_pixel_in(dest, 2, 0, Color::BLUE);
        let src = Region { width: 3, ..dest };
        screen.blit_in(dest, src, 1, 0);
        assert_eq!(screen.get_pixel_in(dest, 1, 0), Some(Color::RED));
        assert_eq!(screen.get_pixel_in(dest, 2, 0), Some(Color::GREEN));
        assert_eq!(screen.get_pixel_in(dest, 3, 0), Some(Color::BLUE));
    }

    #[test]
    fn overlapping_repeat_samples_before_writes() {
        let mut screen = Screen::new();
        let allocation = screen.allocate(4, 1).unwrap();
        let dest = allocation.region;
        screen.set_pixel_in(dest, 0, 0, Color::RED);
        screen.set_pixel_in(dest, 1, 0, Color::GREEN);
        let src = Region { width: 2, ..dest };
        screen.rect_texture_in(dest, 1, 0, 3, 1, src, Mapping::Repeat);
        assert_eq!(screen.get_pixel_in(dest, 1, 0), Some(Color::RED));
        assert_eq!(screen.get_pixel_in(dest, 2, 0), Some(Color::GREEN));
        assert_eq!(screen.get_pixel_in(dest, 3, 0), Some(Color::RED));
    }

    #[test]
    fn frame_pages_preserve_setup_and_previous_frames() {
        let mut screen = Screen::new();
        assert_eq!((VRAM_WIDTH, VRAM_HEIGHT), (960, 480));
        assert_eq!(screen.vram.len(), VRAM_WIDTH * VRAM_HEIGHT);
        assert_eq!((screen.drawing, screen.displayed), (0, 0));
        assert!(
            screen
                .displayed_rows()
                .all(|row| row.len() == SCREEN_WIDTH && row.iter().all(|&p| p == 0))
        );

        screen.set_pixel(0, 0, Color::RED);
        assert_eq!(screen.displayed_rows().next().unwrap()[0], 0x7c00);
        screen.begin_frame();
        assert_eq!((screen.drawing, screen.displayed), (1, 0));
        assert_eq!(screen.get_pixel(0, 0), Some(Color::RED));
        screen.set_pixel(1, 0, Color::GREEN);
        screen.publish_frame();
        assert_eq!(
            &screen.displayed_rows().next().unwrap()[..3],
            &[0x7c00, 0x03e0, 0]
        );

        screen.begin_frame();
        assert_eq!((screen.drawing, screen.displayed), (0, 1));
        screen.set_pixel(2, 0, Color::BLUE);
        assert_eq!(screen.displayed_rows().next().unwrap()[2], 0);
        screen.publish_frame();
        assert_eq!(
            &screen.displayed_rows().next().unwrap()[..3],
            &[0x7c00, 0x03e0, 0x001f]
        );
        screen.begin_frame();
        assert_eq!((screen.drawing, screen.displayed), (1, 0));
        assert_eq!(screen.get_pixel(2, 0), Some(Color::BLUE));
    }

    #[test]
    fn drawing_is_confined_to_current_page() {
        let mut screen = Screen::new();
        screen.fill(Color::RED);
        screen.begin_frame();
        screen.clear();
        screen.rect(-1, -1, 2, 2, Color::WHITE);
        assert_eq!(screen.get_pixel(0, 0), Some(Color::WHITE));
        assert_eq!(screen.get_pixel(1, 0), Some(Color::BLACK));
        assert_eq!(screen.vram[0], 0x7c00);
        assert_eq!(screen.vram[SCREEN_WIDTH * 2], 0);
        assert_eq!(screen.vram[SCREEN_HEIGHT * VRAM_WIDTH], 0);
    }
    #[test]
    fn region_drawing_clips_without_touching_neighbors_or_framebuffer() {
        let mut screen = Screen::new();
        let allocation = screen.allocate(3, 2).unwrap();
        let region = allocation.region;
        screen.fill(Color::RED);
        screen.fill_in(region, Color::BLUE);
        assert_eq!(screen.get_pixel_in(region, 2, 1), Some(Color::BLUE));
        screen.set_pixel_in(region, 3, 0, Color::WHITE);
        assert_eq!(screen.get_pixel_in(region, 3, 0), None);
        screen.rect_in(region, -1, -1, 2, 2, Color::WHITE);
        screen.circle_in(region, 2, 1, 0, Color::GREEN);
        screen.line_in(region, i64::MIN, 1, i64::MAX, 1, Color::WHITE);
        assert_eq!(screen.get_pixel_in(region, 0, 0), Some(Color::WHITE));
        assert_eq!(screen.get_pixel_in(region, 1, 0), Some(Color::BLUE));
        assert_eq!(screen.get_pixel_in(region, 2, 1), Some(Color::WHITE));
        assert_eq!(screen.get_pixel(0, 0), Some(Color::RED));
        assert_eq!(
            screen.vram[region.y * VRAM_WIDTH + region.x + region.width],
            0
        );
        assert_eq!(
            screen.vram[(region.y + region.height) * VRAM_WIDTH + region.x],
            0
        );
        screen.begin_frame();
        assert_eq!(screen.get_pixel_in(region, 0, 0), Some(Color::WHITE));
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
