use super::{Button, SharedInput};
use sdl3::GamepadSubsystem;
use sdl3::event::{Event, WindowEvent};
use sdl3::gamepad::{Axis, Button as PadButton, Gamepad};
use sdl3::joystick::JoystickId;
use sdl3::keyboard::Scancode;

const PAD_BUTTONS: [(PadButton, Button); 12] = [
    (PadButton::DPadUp, Button::Up),
    (PadButton::DPadDown, Button::Down),
    (PadButton::DPadLeft, Button::Left),
    (PadButton::DPadRight, Button::Right),
    (PadButton::Start, Button::Start),
    (PadButton::Back, Button::Select),
    (PadButton::LeftShoulder, Button::L1),
    (PadButton::RightShoulder, Button::R1),
    (PadButton::North, Button::North),
    (PadButton::West, Button::West),
    (PadButton::East, Button::East),
    (PadButton::South, Button::South),
];

fn key_button(key: Scancode) -> Option<Button> {
    Some(match key {
        Scancode::Up => Button::Up,
        Scancode::Down => Button::Down,
        Scancode::Left => Button::Left,
        Scancode::Right => Button::Right,
        Scancode::Return => Button::Start,
        Scancode::Backspace => Button::Select,
        Scancode::Q => Button::L1,
        Scancode::A => Button::L2,
        Scancode::E => Button::R1,
        Scancode::D => Button::R2,
        Scancode::I => Button::North,
        Scancode::J => Button::West,
        Scancode::L => Button::East,
        Scancode::K => Button::South,
        _ => return None,
    })
}

fn axis_button(axis: Axis, value: i16) -> Option<(Button, bool)> {
    let button = match axis {
        Axis::TriggerLeft => Button::L2,
        Axis::TriggerRight => Button::R2,
        _ => return None,
    };
    Some((button, value >= 16384))
}

pub struct Gamepads {
    subsystem: Option<GamepadSubsystem>,
    active: [Option<(JoystickId, Gamepad)>; 2],
}

impl Gamepads {
    pub fn new(context: &sdl3::Sdl, input: &SharedInput) -> Self {
        let subsystem = context
            .gamepad()
            .map_err(|err| eprintln!("Failed to initialize gamepads: {err}"))
            .ok();
        let mut pads = Self {
            subsystem,
            active: [None, None],
        };
        pads.open_available(input, None);
        pads
    }

    fn open(&mut self, id: JoystickId, slot: usize, input: &SharedInput) -> bool {
        let Some(subsystem) = &self.subsystem else {
            return false;
        };
        match subsystem.open(id) {
            Ok(pad) => {
                let mut inputs = input.0.borrow_mut();
                let state = &mut inputs[slot];
                for (physical, virtual_button) in PAD_BUTTONS {
                    state.set(virtual_button, true, pad.button(physical));
                }
                for axis in [Axis::TriggerLeft, Axis::TriggerRight] {
                    let (button, down) = axis_button(axis, pad.axis(axis)).unwrap();
                    state.set(button, true, down);
                }
                self.active[slot] = Some((id, pad));
                true
            }
            Err(err) => {
                eprintln!("Failed to open gamepad {id:?}: {err}");
                false
            }
        }
    }

    fn open_available(&mut self, input: &SharedInput, excluded: Option<JoystickId>) {
        let Some(subsystem) = &self.subsystem else {
            return;
        };
        match subsystem.gamepads() {
            Ok(ids) => {
                for id in ids {
                    if Some(id) == excluded || self.slot(id).is_some() {
                        continue;
                    }
                    let Some(slot) = self.active.iter().position(Option::is_none) else {
                        break;
                    };
                    self.open(id, slot, input);
                }
            }
            Err(err) => eprintln!("Failed to enumerate gamepads: {err}"),
        }
    }

    fn slot(&self, id: JoystickId) -> Option<usize> {
        self.active
            .iter()
            .position(|pad| pad.as_ref().is_some_and(|(active, _)| *active == id))
    }

    pub fn event(&mut self, event: &Event, input: &SharedInput) {
        match *event {
            Event::KeyDown {
                scancode: Some(key),
                repeat: false,
                ..
            } => {
                if let Some(button) = key_button(key) {
                    input.0.borrow_mut()[0].set(button, false, true);
                }
            }
            Event::KeyUp {
                scancode: Some(key),
                ..
            } => {
                if let Some(button) = key_button(key) {
                    input.0.borrow_mut()[0].set(button, false, false);
                }
            }
            Event::Window {
                win_event: WindowEvent::FocusLost,
                ..
            } => input.0.borrow_mut()[0].clear_keyboard(),
            Event::GamepadAdded { which, .. } if self.slot(which).is_none() => {
                if let Some(slot) = self.active.iter().position(Option::is_none) {
                    self.open(which, slot, input);
                }
            }
            Event::GamepadRemoved { which, .. } => {
                if let Some(slot) = self.slot(which) {
                    self.active[slot] = None;
                    input.0.borrow_mut()[slot].clear_gamepad();
                    self.open_available(input, Some(which));
                }
            }
            Event::GamepadButtonDown { which, button, .. }
            | Event::GamepadButtonUp { which, button, .. } => {
                if let Some(slot) = self.slot(which)
                    && let Some((_, virtual_button)) =
                        PAD_BUTTONS.iter().find(|(physical, _)| *physical == button)
                {
                    input.0.borrow_mut()[slot].set(
                        *virtual_button,
                        true,
                        matches!(event, Event::GamepadButtonDown { .. }),
                    );
                }
            }
            Event::GamepadAxisMotion {
                which, axis, value, ..
            } => {
                if let Some(slot) = self.slot(which)
                    && let Some((button, down)) = axis_button(axis, value)
                {
                    input.0.borrow_mut()[slot].set(button, true, down);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_mappings_and_trigger_threshold() {
        let keys = [
            Scancode::Up,
            Scancode::Down,
            Scancode::Left,
            Scancode::Right,
            Scancode::Return,
            Scancode::Backspace,
            Scancode::Q,
            Scancode::A,
            Scancode::E,
            Scancode::D,
            Scancode::I,
            Scancode::J,
            Scancode::L,
            Scancode::K,
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
        for (key, button) in keys.into_iter().zip(buttons) {
            assert_eq!(key_button(key), Some(button));
        }
        assert_eq!(key_button(Scancode::Escape), None);
        assert_eq!(key_button(Scancode::P), None);
        for axis in [Axis::TriggerLeft, Axis::TriggerRight] {
            assert!(!axis_button(axis, 16383).unwrap().1);
            assert!(axis_button(axis, 16384).unwrap().1);
        }
        assert_eq!(axis_button(Axis::LeftX, 32767), None);
    }
    #[test]
    fn keyboard_events_ignore_repeat_and_clear_on_focus_loss() {
        let input = SharedInput::default();
        let mut pads = Gamepads {
            subsystem: None,
            active: [None, None],
        };
        let key = |repeat| Event::KeyDown {
            timestamp: 0,
            window_id: 1,
            keycode: None,
            scancode: Some(Scancode::Left),
            keymod: sdl3::keyboard::Mod::NOMOD,
            repeat,
            which: 0,
            raw: 0,
        };
        pads.event(&key(true), &input);
        assert!(!input.0.borrow()[0].down(Button::Left));
        pads.event(&key(false), &input);
        input.0.borrow_mut()[0].end_frame();
        pads.event(&key(true), &input);
        assert!(input.0.borrow()[0].down(Button::Left));
        assert!(!input.0.borrow()[0].pressed(Button::Left));
        input.0.borrow_mut()[0].set(Button::Left, true, true);
        pads.event(
            &Event::Window {
                timestamp: 0,
                window_id: 1,
                win_event: WindowEvent::FocusLost,
            },
            &input,
        );
        assert!(input.0.borrow()[0].down(Button::Left));
        input.0.borrow_mut()[0].clear_gamepad();
        assert!(!input.0.borrow()[0].down(Button::Left));
    }

    #[test]
    fn two_gamepads_keep_slots_and_independent_states() {
        use sdl3::joystick::{JoystickType, VirtualJoystickDescription};
        let sdl = sdl3::init().unwrap();
        let joystick = sdl.joystick().unwrap();
        let subsystem = sdl.gamepad().unwrap();
        let attach = || {
            joystick
                .attach_virtual_joystick(
                    VirtualJoystickDescription::new()
                        .joystick_type(JoystickType::Gamepad)
                        .with_button(PadButton::DPadLeft),
                )
                .unwrap()
        };
        let first = attach();
        let second = attach();
        let input = SharedInput::default();
        let mut pads = Gamepads {
            subsystem: Some(subsystem),
            active: [None, None],
        };
        pads.event(
            &Event::GamepadAdded {
                timestamp: 0,
                which: first.id(),
            },
            &input,
        );
        pads.event(
            &Event::GamepadAdded {
                timestamp: 0,
                which: second.id(),
            },
            &input,
        );
        assert_eq!(pads.slot(first.id()), Some(0));
        assert_eq!(pads.slot(second.id()), Some(1));
        pads.event(
            &Event::GamepadButtonDown {
                timestamp: 0,
                which: second.id(),
                button: PadButton::DPadLeft,
            },
            &input,
        );
        assert!(!input.0.borrow()[0].down(Button::Left));
        assert!(input.0.borrow()[1].down(Button::Left));
        let first_id = first.id();
        drop(first);
        pads.event(
            &Event::GamepadRemoved {
                timestamp: 0,
                which: first_id,
            },
            &input,
        );
        assert_eq!(pads.slot(second.id()), Some(1));
        assert!(input.0.borrow()[1].down(Button::Left));
        let replacement = attach();
        pads.event(
            &Event::GamepadAdded {
                timestamp: 0,
                which: replacement.id(),
            },
            &input,
        );
        assert_eq!(pads.slot(replacement.id()), Some(0));
        assert_eq!(pads.slot(second.id()), Some(1));
    }
}
