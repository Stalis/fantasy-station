use std::{env, fs, path::PathBuf};

use shaderc::{Compiler, ShaderKind};
use spirv_cross2::{
    Compiler as CrossCompiler, Module,
    compile::{
        CompilableTarget,
        msl::{BindTarget, ResourceBinding},
    },
    spirv::ExecutionModel,
    targets::Msl,
};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
    let metal = env::var("CARGO_CFG_TARGET_OS").expect("target OS is not set") == "macos";
    let compiler = Compiler::new().expect("failed to initialize shaderc");

    for (name, kind, stage) in [
        (
            "fullscreen.vert",
            ShaderKind::Vertex,
            ExecutionModel::Vertex,
        ),
        (
            "classic_soft.frag",
            ShaderKind::Fragment,
            ExecutionModel::Fragment,
        ),
        (
            "passthrough.frag",
            ShaderKind::Fragment,
            ExecutionModel::Fragment,
        ),
    ] {
        let path = format!("shaders/src/{name}");
        println!("cargo:rerun-if-changed={path}");
        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
        let spirv = compiler
            .compile_into_spirv(&source, kind, &path, "main", None)
            .unwrap_or_else(|e| panic!("failed to compile {path} to SPIR-V: {e}"));
        fs::write(out_dir.join(format!("{name}.spv")), spirv.as_binary_u8())
            .unwrap_or_else(|e| panic!("failed to write SPIR-V for {path}: {e}"));

        if metal {
            let mut cross = CrossCompiler::<Msl>::new(Module::from_words(spirv.as_binary()))
                .unwrap_or_else(|e| panic!("failed to load SPIR-V for {path}: {e}"));
            cross
                .rename_entry_point("main", "shader_main", stage)
                .unwrap_or_else(|e| panic!("failed to rename MSL entry point for {path}: {e}"));
            if stage == ExecutionModel::Fragment {
                cross
                    .add_resource_binding(
                        stage,
                        ResourceBinding::from_qualified(2, 0),
                        &BindTarget {
                            buffer: 0,
                            texture: 0,
                            sampler: 0,
                            count: None,
                        },
                    )
                    .unwrap_or_else(|e| panic!("failed to bind MSL texture for {path}: {e}"));
            }
            let msl = cross
                .compile(&Msl::options())
                .unwrap_or_else(|e| panic!("failed to compile {path} to MSL: {e}"));
            let mut source = msl.as_ref().as_bytes().to_vec();
            source.push(0);
            fs::write(out_dir.join(format!("{name}.msl")), source)
                .unwrap_or_else(|e| panic!("failed to write MSL for {path}: {e}"));
        }
    }
}
