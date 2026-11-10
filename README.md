# Fantasy Station

**A constrained fantasy console for building and playing games.**

Fantasy Station is a virtual game platform inspired by the creative constraints of [PICO-8](https://www.lexaloffle.com/pico-8.php) and the visual character of late-1990s consoles. It provides a predictable runtime, a versioned game API, portable disks, and a command-line SDK — **without a built-in IDE**.

The goal is not to emulate PlayStation hardware. Instead, Fantasy Station defines its own small, deliberately limited platform: games run inside its rules, regardless of the host machine.

> **Status: early design / prototype.** This README describes the intended direction. Features, limits, file formats, and CLI commands below are proposals unless explicitly implemented. The FS-1 specification is not frozen.

## Principles

- **Constraints encourage creativity.** Games target a documented virtual machine with fixed, enforceable resource budgets.
- **Use your own tools.** Write code in any editor; use Git, external asset pipelines, and normal development workflows.
- **Build once, play anywhere.** Keep game code and assets together on a disk.
- **Separate game output from display output.** The game generates a clean virtual framebuffer; the player controls optional display effects.
- **Compatibility is a contract.** Disk manifests may eventually declare a platform profile and required capabilities.
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
| Disk | Directory with a JSON manifest and game files |

These figures describe the **virtual platform**, not the requirements or capabilities of the host computer. Memory layout, accounting rules, sprite/texture constraints, and the CPU cost model still need to be specified. Lua heap accounting may be distinct from virtual addressable RAM.

## Display philosophy

Fantasy Station intentionally supports old-school rendering tricks such as ordered dithering, fake transparency, and palette-based shading.

The default player-facing display preset is **Classic Soft**:

- subtle blending between neighboring virtual pixels, helping dither patterns visually merge;
- light, soft bloom around bright areas.

Press **P** to switch between Classic Soft and **Pixel Perfect** (no post-processing). The current preset appears in the window title. A more pronounced **CRT** look remains planned.

**Display post-processing belongs to the player, not the disk.** It must not alter the game's framebuffer, input, simulation, virtual resource budgets, or API behavior. Games should remain usable with post-processing disabled. A game may recommend a preset, but must not force it.

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
                              disk directory (manifest + files)
                                            |
                                            v
                                     Fantasy Runtime
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
- **SDK / CLI:** proposed tools for creating, validating, and running disks.
- **Disk:** a directory containing a JSON manifest and game files.

The Rust source separates the application loop (`src/main.rs`), disk loading (`src/disk.rs`), screen state (`src/screen.rs`), Lua integration (`src/script/`), input state and SDL devices (`src/input/`), and GPU rendering (`src/renderer.rs`).

SDL3 and host GPU objects are implementation details, not part of the game-facing API.

## Feature Packs

Feature Packs are **versioned capability contracts**, not arbitrary native plugins shipped by games. A disk may eventually require or optionally use packs supported by its target runtime.

Examples of *possible* packs:

- `graphics.sprite@1`
- `graphics.dither@1`
- `input.gamepad@1`
- `audio.stream@1`
- `graphics.3d@1` (future)

FS-1 will define a minimal core capability set. Later profiles, including FS-2, can add capabilities without unnecessarily breaking existing disks. Feature declarations and the registry are not implemented.

## External development workflow

Fantasy Station **will not include an IDE**. You can write Lua in Zed, VS Code, Neovim, Rider, or any other editor.

Run the bundled demo with `cargo run` or `cargo run -- disks/demo`; pass another disk directory to run a different game. The prototype requires an SDL GPU device supporting Metal on macOS or SPIR-V on other platforms. Shaders are compiled during the Rust build.

The prototype calls optional Lua `setup()` once, then processes input and calls `update()` followed by `draw()` each frame. `draw()` runs last, immediately before the completed screen is rendered. The old `loop()` callback is not called.

Proposed CLI commands (not yet guaranteed to exist):

```sh
fs new my-game       # Scaffold a project
cd my-game
fs run               # Run the current disk
fs inspect .         # Inspect a disk
```

Development tools should enforce the same virtual hardware limits as the runtime. Debugging and file watching may be provided by the SDK without becoming game capabilities.

### Current graphics API

The prototype exposes the framebuffer through the global `graphics` object. Its
`graphics.width` and `graphics.height` properties give the dimensions in pixels.
Coordinates and sizes are integers, with `(0, 0)` at the top left. Drawing is
clipped to the framebuffer.

Colors are opaque packed RGB555 integers in the range `0` to `0x7fff`: bits
14–10 are red, bits 9–5 are green, and bits 4–0 are blue. There is no alpha
channel. For example, black is `0x0000`, red is `0x7c00`, green is `0x03e0`,
blue is `0x001f`, and white is `0x7fff`. Out-of-range colors raise a Lua error.

```lua
graphics:clear()                         -- Fill the framebuffer with black.
graphics:fill(0x7c00)                    -- Fill it with red.
graphics:set_pixel(x, y, 0x7fff)         -- Set one pixel.
local color = graphics:get_pixel(x, y)   -- Packed color, or nil outside.
graphics:circle(cx, cy, radius, 0x001f)  -- Filled circle.
graphics:rect(x, y, width, height, 0x03e0) -- Filled rectangle.
graphics:line(x0, y0, x1, y1, 0x7fff)  -- Line including both endpoints.
```

Rectangles with zero or negative width or height and circles with negative
radius draw nothing; a circle with radius zero draws its center pixel.

### Current input API

The prototype exposes two virtual digital gamepads through the global `input` object:

```lua
local controller = input:get_controller() -- same as input:get_controller(0)
local second = input:get_controller(1)

if controller:down("left") then
    -- Held this frame.
end
if controller:pressed("south") then
    -- Newly pressed this frame; repeated calls return the same result.
end
if controller:released("south") then
    -- Released this frame.
end
if second:down("left") then
    -- Player 2 is holding left.
end
```

`get_controller` accepts an optional zero-based index (`0` or `1`), defaulting to `0`. Controller methods return a boolean. `pressed` and `released` record transitions of the combined input until `draw()` completes; both `update()` and `draw()` see the same events. A quick press and release can set both. Releasing one source while another holds the button does not count as a release; focus loss or controller disconnection can produce one. `setup()` sees all buttons released. Unknown button names or controller indices raise a Lua error. Controller objects remain valid when devices disconnect or change.

| Button name | Keyboard (physical keys) | SDL gamepad |
| --- | --- | --- |
| `up`, `down`, `left`, `right` | Arrow keys | D-pad |
| `start`, `select` | Enter, Backspace | Start, Back |
| `l1`, `l2` | Q, A | Left shoulder, left trigger |
| `r1`, `r2` | E, D | Right shoulder, right trigger |
| `north`, `west`, `east`, `south` | I, J, L, K | Corresponding face positions |

Triggers become pressed at SDL axis value 16384 or above. The first two available SDL gamepads occupy player slots 1 and 2; keyboard input is combined with player 1 only. Other gamepads wait for a free slot. When a gamepad disconnects, the other stays in its slot and an available replacement fills the vacant slot. Losing window focus releases keyboard input. Escape exits and P changes the display preset.

The `fs.input` shown below is a separate future API, not the implemented `input` object.

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

## Disk format

A disk is a directory with a `manifest.json` and its game files:

```text
my-disk/
├── manifest.json
└── main.lua
```

```json
{
  "name": "My Game",
  "entrypoint": "main.lua"
}
```

Both fields are required strings. `name` must not be empty; `entrypoint` is a path relative to the disk directory and must resolve inside it. Additional manifest fields are ignored for future expansion. The disk name appears in the window title. The runtime currently loads only the entrypoint; sprites, MIDI files, and other assets are planned, not yet supported.

Disks are not sandboxed: only run Lua code from trusted sources.

## Roadmap

### FS-1 — 2D foundation

1. Rust runtime with SDL3 window, input, and a fixed-size framebuffer.
2. Basic 2D graphics API and a Lua VM integration.
3. Virtual hardware limits and a measurable execution budget.
4. Player-controlled display presets, starting with pixel blending and soft bloom.
5. SDK commands for creating, inspecting, and running disks.
6. Manifest validation, capability discovery, and compatibility tests.

### FS-2 — future 3D expansion

- 3D geometry and cameras;
- textured triangle rendering and a constrained virtual GPU interface;
- PS1-era-inspired visual constraints;
- continued support for compatible FS-1 disks.

## Non-goals

- A custom IDE or integrated code/asset editor.
- Cycle-accurate PlayStation or MIPS hardware emulation.
- Unrestricted native plugins embedded in game disks.
- A general-purpose game engine without hardware limits.

## Project status

Fantasy Station is currently defining its core architecture and first specification. APIs, resource limits, and disk details will evolve as the prototype is implemented.

## License

To be determined.
