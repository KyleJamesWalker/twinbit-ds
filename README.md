# twinbit-ds

A Rust tech demo for the ANBERNIC RG DS Plus. It is a test project, not a finished game. It gives a working example of every hardware feature, as a starting point for a real game.

## Overview

The demo is a short run of dual-screen microgames, a 3D dice tray with rigid body physics, and a live button tester. Together they use both screens, multi-touch, the microphone, every button and the stick. They also use both speakers, the vibration motor, the lid sensor and the power button. The same code also builds and runs on macOS for quick iteration.

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

Install into the stock menu as Ports > Twinbit:

```sh
scripts/install.sh
```

### Debug mode

`DEBUG=1 scripts/deploy.sh` starts the game with a control pipe at `/tmp/twinbit.ctl`. `scripts/debug.sh` drives it over SSH:

```sh
scripts/debug.sh scene dice                 # open a scene
scripts/debug.sh press a 120                # hold a button for 120 ms
scripts/debug.sh tap 500 400                # touch the bottom screen
scripts/debug.sh swipe 300 650 650 250 120  # drag over 120 ms
scripts/debug.sh shot out.png               # save both screens, top above bottom
scripts/debug.sh logs                       # follow the game log
```

On macOS, set `TWINBIT_DEBUG=1` and write commands to the pipe directly. Set `TWINBIT_START=dice` to open the dice scene at launch.

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
| Sleep and wake | Power button | None |
| Back to title | Back | Tab |
| Quit | Hold Home for one second | Esc |

On the title screen, every button plays a tone. L1 and R1 play a tone from the left and right speaker, R2 runs the vibration motor, and Y opens the dice scene. In the dice scene, tap, swipe or press A to throw the die.

## Layout

| Path | Contents |
|---|---|
| `crates/game` | Game logic, input model, microgames, dice physics, 3D meshes, bitmap font. No platform dependencies. |
| `crates/render` | OpenGL renderer for 2D shapes and lit 3D meshes: GL 3.3 on macOS, GLES 3.0 on the device. |
| `crates/app` | Windows, input, audio, mic, debug mode, and the device-only gamepad, lid and motor code. |
| `scripts` | Cross-compile, sysroot, deploy, install and debug scripts, and the on-device run script. |
| `assets` | The Ports menu launcher and its cover art. |

## Device notes

The stock firmware runs Weston with one output per panel. DSI-1 is the top screen and DSI-2 is the bottom screen. The game swaps them when the system screen-swap setting (`lcdswap`) is on. Set `TWINBIT_TOP` to override the assignment.

| Hardware | Interface |
|---|---|
| Buttons, stick, d-pad | evdev `ANBERNIC-rk3568-keys`. Weston does not forward this device. |
| Back key | evdev `adc-keys`, `KEY_BACK` |
| Power key | evdev `rk805 pwrkey`, `KEY_POWER`. A press runs the stock `/mnt/vendor/ctrl/pwr_new.sh` to suspend. |
| Touch | Wayland touch events from the `gt9xx` panel, up to 10 fingers |
| Lid | `/sys/class/anbernic_misc/hallkey`: `3` open, `2` closed |
| Vibration motor | `/sys/class/anbernic_misc/moto`: `1` on, `0` off |
| Microphone and speaker | ALSA through PulseAudio, `default` device |

The button codes do not follow the evdev gamepad layout. For example, Start reports `BTN_TR` and A reports `BTN_SOUTH`. `crates/app/src/device.rs` holds the full map.

When the stock menu launches a port, the frontend exits first and restarts afterwards. `scripts/deploy.sh` runs the game over SSH instead, so the on-device `run.sh` pauses the frontend to stop its idle timer from suspending the device.
