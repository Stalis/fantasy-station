use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Button {
    Up,
    Down,
    Left,
    Right,
    Start,
    Select,
    L1,
    L2,
    R1,
    R2,
    North,
    West,
    East,
    South,
}

impl Button {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "up" => Self::Up,
            "down" => Self::Down,
            "left" => Self::Left,
            "right" => Self::Right,
            "start" => Self::Start,
            "select" => Self::Select,
            "l1" => Self::L1,
            "l2" => Self::L2,
            "r1" => Self::R1,
            "r2" => Self::R2,
            "north" => Self::North,
            "west" => Self::West,
            "east" => Self::East,
            "south" => Self::South,
            _ => return None,
        })
    }

    fn mask(self) -> u16 {
        1 << self as u8
    }
}

#[derive(Default)]
pub struct Input {
    keyboard: u16,
    gamepad: u16,
    pressed: u16,
    released: u16,
}

impl Input {
    pub fn down(&self, button: Button) -> bool {
        (self.keyboard | self.gamepad) & button.mask() != 0
    }

    pub fn pressed(&self, button: Button) -> bool {
        self.pressed & button.mask() != 0
    }

    pub fn released(&self, button: Button) -> bool {
        self.released & button.mask() != 0
    }

    pub fn set(&mut self, button: Button, gamepad: bool, down: bool) {
        let before = self.keyboard | self.gamepad;
        let source = if gamepad {
            &mut self.gamepad
        } else {
            &mut self.keyboard
        };
        if down {
            *source |= button.mask();
        } else {
            *source &= !button.mask();
        }
        self.pressed |= !before & (self.keyboard | self.gamepad);
        self.released |= before & !(self.keyboard | self.gamepad);
    }

    pub fn clear_keyboard(&mut self) {
        self.released |= self.keyboard & !self.gamepad;
        self.keyboard = 0;
    }
    pub fn clear_gamepad(&mut self) {
        self.released |= self.gamepad & !self.keyboard;
        self.gamepad = 0;
    }
    pub fn end_frame(&mut self) {
        self.pressed = 0;
        self.released = 0;
    }
}

#[derive(Clone, Default)]
pub struct SharedInput(pub Rc<RefCell<[Input; 2]>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_exact() {
        let names = [
            "up", "down", "left", "right", "start", "select", "l1", "l2", "r1", "r2", "north",
            "west", "east", "south",
        ];
        let buttons = [
            Button::Up,
            Button::Down,
            Button::Left,
            Button::Right,
            Button::Start,
            Button::Select,
            Button::L1,
            Button::L2,
            Button::R1,
            Button::R2,
            Button::North,
            Button::West,
            Button::East,
            Button::South,
        ];
        for (name, button) in names.into_iter().zip(buttons) {
            assert_eq!(Button::parse(name), Some(button));
        }
        assert_eq!(Button::parse("LEFT"), None);
    }

    #[test]
    fn releases_track_combined_edges_until_frame_end() {
        let mut input = Input::default();
        let b = Button::Left;
        input.set(b, false, true);
        input.set(b, true, true);
        input.end_frame();
        input.set(b, false, false);
        assert!(!input.released(b));
        input.clear_gamepad();
        assert!(input.released(b) && input.released(b));
        input.set(b, false, true);
        assert!(input.down(b) && input.pressed(b) && input.released(b));
        input.end_frame();
        assert!(!input.released(b));
        input.clear_keyboard();
        assert!(input.released(b));
        input.end_frame();
        input.set(b, false, false);
        assert!(!input.released(b));
    }

    #[test]
    fn transitions_merge_sources_and_survive_release() {
        let mut input = Input::default();
        let b = Button::Left;
        input.set(b, false, true);
        assert!(input.down(b) && input.pressed(b) && input.pressed(b));
        input.end_frame();
        input.set(b, false, true);
        input.set(b, true, true);
        assert!(input.down(b) && !input.pressed(b));
        input.clear_keyboard();
        assert!(input.down(b));
        input.clear_gamepad();
        assert!(!input.down(b));
        input.set(b, false, true);
        input.set(b, false, false);
        assert!(!input.down(b) && input.pressed(b));
        input.end_frame();
        assert!(!input.pressed(b));
        input.set(b, false, true);
        input.set(b, true, true);
        input.clear_gamepad();
        assert!(input.down(b));
        input.clear_keyboard();
        assert!(!input.down(b));
    }
}
