use mlua::{UserData, UserDataMethods};

use crate::input::{Button, SharedInput};

struct Controller {
    input: SharedInput,
    slot: usize,
}

impl UserData for SharedInput {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_controller", |_, this, index: Option<i64>| {
            let index = index.unwrap_or(0);
            match index {
                0 | 1 => Ok(Controller {
                    input: this.clone(),
                    slot: index as usize,
                }),
                _ => Err(mlua::Error::RuntimeError(format!(
                    "invalid controller index: {index}; expected 0 or 1"
                ))),
            }
        });
    }
}

impl UserData for Controller {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("down", |_, this, name: String| {
            let button = Button::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("unknown input button: {name}"))
            })?;
            Ok(this.input.0.borrow()[this.slot].down(button))
        });
        methods.add_method("pressed", |_, this, name: String| {
            let button = Button::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("unknown input button: {name}"))
            })?;
            Ok(this.input.0.borrow()[this.slot].pressed(button))
        });
        methods.add_method("released", |_, this, name: String| {
            let button = Button::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("unknown input button: {name}"))
            })?;
            Ok(this.input.0.borrow()[this.slot].released(button))
        });
    }
}
