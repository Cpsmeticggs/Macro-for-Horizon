# Macro Keybinds for Horizon

A native [Horizon](https://github.com/horizon-rust/horizon) port of **Macro Keybinds 2.0.0** by Thomas7520, converted from Fabric with `horizon-port`.

Create macros, bind them to keys, and fire them from a radial macro wheel — no Fabric API required.

## Features

- **Macro types:** Simple, Toggle, Repeat, Delayed, Counted Repeat, Alternate
- **Actions:** Send Message, Execute Command, Fill Chat Without Send, Display Local Message
- **Scopes:** Global macros and per-server macros
- **Macro wheel:** radial menu for quick access, hold or toggle
- **Key binds:** dedicated keys to open the options screen and the macro wheel
- **In-game UI:** full config and macro editor screens
- **Localization:** English, Spanish, French, Russian

## Install

1. Download `macrokeybinds-horizon.hnmod` from [Releases](https://github.com/Priyanshu7037/Macro-Keybinds-for-Horizon/releases).
2. Drop it into Horizon's native mods folder (`%APPDATA%\horizon\horizon-native-mods` on Windows).
3. Launch the game and open **Options > Mods** to configure it.

Runs on Minecraft **26.3** with Horizon revision `fdc87ae`.

## Build

Horizon's launcher builds the bundle itself (it downloads its own Rust toolchain the first time):

```bash
horizon-launcher native-build "<this folder>" --trust-native-code
```

Tests for the translated logic run without the game:

```bash
cargo test
```

## Project layout

| Path | Purpose |
|---|---|
| `mixins/macrokeybinds/runtime.rs` | Translated mod logic (`pub mod java`) and tests |
| `mixins/macrokeybinds/game_side.rs` | Game-facing side: player, chat, HUD, keys |
| `mixins/macrokeybinds/engine.rs` | Macro execution engine |
| `mixins/macrokeybinds/*.rs` | UI, screens, language, and hook helpers |
| `mixins/macrokeybinds/horizon-mixin.toml` | Hook and module definitions |
| `PORTING.md` | Port report: what was translated, what remains |

## Compatibility

- Game: **26.3**
- Horizon: `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca`
- Horizon mixins: `5a64af938ec86b00d53d16a408aabc0038fb3e57`
- Hook count: 404

Raw hook targets are tied to one Horizon commit; a newer Nightly may move them and fail the build with a clear message rather than applying silently.

## Licence

Original mod: **Attribution-NonCommercial-ShareAlike 4.0 International (CC BY-NC-SA 4.0)**, by Thomas7520. This port keeps the same licence. See [PORTING.md](PORTING.md) for provenance details.
