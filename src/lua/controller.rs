use mlua::{UserData, UserDataMethods};

use crate::gateway::Input;

pub(super) struct LuaInput(pub(super) Input);

struct Controller {
    input: Input,
    slot: usize,
}

impl UserData for LuaInput {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_controller", |_, this, index: Option<i64>| {
            let index = index.unwrap_or(0);
            match index {
                0 | 1 => Ok(Controller {
                    input: this.0.clone(),
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
            this.input
                .down(this.slot, &name)
                .ok_or_else(|| mlua::Error::RuntimeError(format!("unknown input button: {name}")))
        });
        methods.add_method("pressed", |_, this, name: String| {
            this.input
                .pressed(this.slot, &name)
                .ok_or_else(|| mlua::Error::RuntimeError(format!("unknown input button: {name}")))
        });
        methods.add_method("released", |_, this, name: String| {
            this.input
                .released(this.slot, &name)
                .ok_or_else(|| mlua::Error::RuntimeError(format!("unknown input button: {name}")))
        });
    }
}
