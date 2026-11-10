# Fantasy Station

**A constrained fantasy console for building and playing games.**

Fantasy Station is a virtual game platform inspired by the creative constraints of [PICO-8](https://www.lexaloffle.com/pico-8.php) and the visual character of late-1990s consoles. It provides a predictable runtime, a versioned game API, portable cartridges, and a command-line SDK — **without a built-in IDE**.

The goal is not to emulate PlayStation hardware. Instead, Fantasy Station defines its own small, deliberately limited platform: games run inside its rules, regardless of the host machine.

> **Status: early design / prototype.** This README describes the intended direction. Features, limits, file formats, and CLI commands below are proposals unless explicitly implemented. The FS-1 specification is not frozen.

## Principles

- **Constraints encourage creativity.** Games target a documented virtual machine with fixed, enforceable resource budgets.
- **Use your own tools.** Write code in any editor; use Git, external asset pipelines, and normal development workflows.
- **Build once, play anywhere.** Package game code and assets into a self-contained cartridge.
- **Separate game output from display output.** The game generates a clean virtual framebuffer; the player controls optional display effects.
- **Compatibility is a contract.** Cartridge manifests declare a platform profile and required capabilities.
- **2D first.** FS-1 focuses on 2D; 3D is deferred to a future FS-2 profile or compatible feature expansion.

## FS-1: proposed baseline

| Capability | Proposed target |
| --- | --- |
| Graphics | 2D primitives, sprites, tilemaps |
| Virtual resolution | 320 × 240 |
| Color | RGB555 (15-bit color) |
| Timing | Fixed-step update; 30/60 Hz modes under consideration |
| Virtual RAM | 2 MiB (provisional) |
| Video memory budget | 1 MiB (provisional) |
| Scripting | Lua; Luau is under evaluation |
| Execution | VM-managed code and a defined per-tick computation budget |
| Input | Virtual gamepad abstraction |
| Sound | Limited, deterministic virtual audio API (details TBD) |
| Cartridge | `.fsc` container with compiled scripts and assets (proposed) |

These figures describe the **virtual platform**, not the requirements or capabilities of the host computer. Memory layout, accounting rules, sprite/texture constraints, and the CPU cost model still need to be specified. Lua heap accounting may be distinct from virtual addressable RAM.

## Display philosophy

Fantasy Station intentionally supports old-school rendering tricks such as ordered dithering, fake transparency, and palette-based shading.

The default player-facing display preset is **Classic Soft**:

- subtle blending between neighboring virtual pixels, helping dither patterns visually merge;
- light, soft bloom around bright areas.

Press **P** to switch between Classic Soft and **Pixel Perfect** (no post-processing). The current preset appears in the window title. A more pronounced **CRT** look remains planned.

**Display post-processing belongs to the player, not the cartridge.** It must not alter the game's framebuffer, input, simulation, virtual resource budgets, or API behavior. Games should remain usable with post-processing disabled. A game may recommend a preset, but must not force it.

The effect uses an SDL3 GPU fullscreen pass. More sophisticated bloom may use separate downsample/blur/composite passes. The rendering path is:

```text
Game (Lua) -> FS-1 2D API -> Virtual framebuffer (320x240)
                                      |
                                      v
                             Host display pipeline
                        pixel blending / bloom / scaling
                                      |
                                      v
                                  SDL3 window
```

## Platform architecture

```text
project source + assets
         |
         v
     Fantasy SDK  ----------------->  .fsc cartridge
     (build / run)                          |
         |                                 v
         +------------------------> Fantasy Runtime
                                            |
                      +---------------------+------------------+
                      |                     |                  |
                  Script VM          Virtual hardware     Feature packs
                  Lua / Luau         2D, audio, input     versioned APIs
                      |                     |
                      +---------------------+
                                            |
                                      SDL3 host layer
                                      window / GPU / audio
```

### Components

- **Runtime (Rust + SDL3):** hosts the virtual console and displays completed frames.
- **Virtual hardware:** defines accessible memory, timing, input, graphics, audio, and limits.
- **Script VM:** runs game logic with access only to the documented Fantasy Station API.
- **SDK / CLI:** creates projects, validates capability declarations, runs development builds, and packages cartridges.
- **Cartridge format:** distributes compiled scripts, resources, and metadata as a portable unit.

SDL3 and host GPU objects are implementation details, not part of the game-facing API.

## Feature Packs

Feature Packs are **versioned capability contracts**, not arbitrary native plugins shipped by games. A cartridge can require or optionally use packs supported by its target runtime.

Examples of *possible* packs:

- `graphics.sprite@1`
- `graphics.dither@1`
- `input.gamepad@1`
- `audio.stream@1`
- `graphics.3d@1` (future)

FS-1 will define a minimal core capability set. Later profiles, including FS-2, can add capabilities without unnecessarily breaking existing cartridges.

A proposed project manifest might look like this:

```toml
[cartridge]
name = "Dither Garden"
version = "0.1.0"
entrypoint = "src/main.lua"

[platform]
target = "fs-1"

[features]
required = ["graphics.sprite@1", "input.gamepad@1"]
optional = ["graphics.dither@1"]

[display]
recommended_preset = "classic-soft"
```

Names and syntax are illustrative; the feature registry and manifest schema are not finalized. Host display post-processing is intentionally separate from game-required capabilities.

## External development workflow

Fantasy Station **will not include an IDE**. You can write Lua in Zed, VS Code, Neovim, Rider, or any other editor.

The current prototype runs with `cargo run -- main.lua`. It requires an SDL GPU device supporting Metal on macOS or SPIR-V on other platforms. Shaders are compiled during the Rust build.

Proposed CLI commands (not yet guaranteed to exist):

```sh
fs new my-game       # Scaffold a project
cd my-game
fs run               # Run from project sources
fs build             # Compile and package a cartridge
fs run dist/game.fsc # Run a packaged cartridge
fs inspect dist/game.fsc
```

Development mode and cartridge mode should enforce the same virtual hardware limits. Debugging and file watching may be provided by the SDK without becoming game capabilities.

### Illustrative game API

```lua
function fs.init()
    x, y = 160, 120
end

function fs.update(dt)
    if fs.input.down("left") then
        x = x - 80 * dt
    end
end

function fs.draw()
    fs.gfx.clear(0x0000)
    fs.gfx.rect(x, y, 12, 12, 0x7fff)
end
```

This API is a sketch, **not implemented API documentation**.

## Compiled cartridges

Fantasy Station intends to compile Lua-family scripts into bytecode and package them alongside assets, for example:

```text
game.fsc
├── manifest.json
├── code/
│   └── main.luac
├── textures/
└── audio/
```

A compiled cartridge is not tied to a particular editor or source directory layout. **Bytecode compatibility must be explicit**: the VM implementation, bytecode version, and validation rules will be recorded in the cartridge format. Standard Lua bytecode is not automatically portable between VM versions or safe to load from untrusted sources; sandboxing, bytecode validation, and resource limits are required before supporting untrusted cartridges.

## Roadmap

### FS-1 — 2D foundation

1. Rust runtime with SDL3 window, input, and a fixed-size framebuffer.
2. Basic 2D graphics API and a Lua VM integration.
3. Virtual hardware limits and a measurable execution budget.
4. Player-controlled display presets, starting with pixel blending and soft bloom.
5. SDK commands for running projects and building `.fsc` cartridges.
6. Manifest validation, capability discovery, and compatibility tests.

### FS-2 — future 3D expansion

- 3D geometry and cameras;
- textured triangle rendering and a constrained virtual GPU interface;
- PS1-era-inspired visual constraints;
- continued support for compatible FS-1 cartridges.

## Non-goals

- A custom IDE or integrated code/asset editor.
- Cycle-accurate PlayStation or MIPS hardware emulation.
- Unrestricted native plugins embedded in game cartridges.
- A general-purpose game engine without hardware limits.

## Project status

Fantasy Station is currently defining its core architecture and first specification. APIs, resource limits, and cartridge details will evolve as the prototype is implemented.

## License

To be determined.
