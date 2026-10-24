# twinbit-ds

A Rust tech demo for the ANBERNIC RG DS Plus. It is a test project, not a finished game. It gives a working example of every hardware feature, as a starting point for a real game.

## Overview

The demo is a short run of dual-screen microgames plus a live button tester. Together they use both screens, the touch panel, the microphone, every button and the stick, the speaker, the vibration motor and the lid sensor. The same code also builds and runs on macOS for quick iteration.

## Requirements

- Rust 1.94 or later, with the `aarch64-unknown-linux-gnu` target.
- [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) and [Zig](https://ziglang.org) for the device build.
- An RG DS Plus on the stock firmware, with SSH turned on and reachable as `anbernic.local`. Set `RGDS_HOST` to use a different host.
- Key-based SSH login to the device. The stock root password is `root`, so run `ssh-copy-id root@anbernic.local` once.

## Setup

```sh
rustup target add aarch64-unknown-linux-gnu
scripts/sysroot.sh        # copy the device's libasound for linking
```

## Usage

Run on macOS:

```sh
cargo run -p twinbit
```

Build, copy and start on the device:

```sh
scripts/deploy.sh
CAPTURE=1 scripts/deploy.sh   # also log every input device and the lid sensor
ssh root@anbernic.local tail -f /userdata/twinbit/run.log
```

Run the tests:

```sh
cargo test
```

### Controls

| Action | Device | macOS |
|---|---|---|
| D-pad | D-pad or stick | Arrow keys |
| A / B / X / Y | A / B / X / Y | X / Z / S / A |
| L1 / R1 / L2 / R2 | Shoulders | Q / W / 1 / 2 |
| Start / Select | Start / Select | Enter / Backspace |
| Touch | Stylus on the bottom screen | Mouse on the bottom window |
| Blow | Hinge microphone | Microphone, or hold Space |
| Volume | V+ / V- | = / - |
| Lid | Close the lid | C toggles |
| Back to title | Back | Tab |
| Quit | Hold Home for one second | Esc |

On the title screen, every button plays a tone and R2 runs the vibration motor.

## Layout

| Path | Contents |
|---|---|
| `crates/game` | Game logic, input model, microgames, bitmap font. No platform dependencies. |
| `crates/render` | OpenGL renderer for rectangles and circles: GL 3.3 on macOS, GLES 3.0 on the device. |
| `crates/app` | Windows, input, audio, mic, and the device-only gamepad, lid and motor code. |
| `scripts` | Cross-compile, sysroot and deploy scripts, and the on-device run script. |

## Device notes

The stock firmware runs Weston with one output per panel. DSI-1 is the top screen and DSI-2 is the bottom screen. Set `TWINBIT_TOP` to change the assignment.

| Hardware | Interface |
|---|---|
| Buttons, stick, d-pad | evdev `ANBERNIC-rk3568-keys`. Weston does not forward this device. |
| Back key | evdev `adc-keys`, `KEY_BACK` |
| Touch | Wayland touch events from the `gt9xx` panel |
| Lid | `/sys/class/anbernic_misc/hallkey`: `3` open, `2` closed |
| Vibration motor | `/sys/class/anbernic_misc/moto`: `1` on, `0` off |
| Microphone and speaker | ALSA through PulseAudio, `default` device |

The button codes do not follow the evdev gamepad layout. For example, Start reports `BTN_TR` and A reports `BTN_SOUTH`. `crates/app/src/device.rs` holds the full map.
