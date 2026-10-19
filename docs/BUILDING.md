# Building SciCalc 82

This page covers building and testing the app and its installers. To learn how to use the calculator, see the [User Manual](USER_MANUAL.md).

## Contents

- [Run it](#run-it)
- [The web version](#the-web-version)
- [Installers](#installers)
- [Tests and screenshots](#tests-and-screenshots)
- [Project layout](#project-layout)

## Run it

You need a recent stable Rust toolchain (edition 2024).

```bash
cargo run --release -p calc-app
```

The optimized executable is written to `target/release/scicalc82.exe` (`scicalc82` on macOS and Linux).

On Windows, the build also embeds the app icon in the exe, so Explorer and shortcuts show it. This needs the resource compiler from the Windows SDK. Without it, the build still succeeds and prints a warning, but the exe has no icon.

## The web version

The same app runs in a browser as WebAssembly. You need the `wasm32-unknown-unknown` target and [trunk](https://trunkrs.dev):

```bash
rustup target add wasm32-unknown-unknown
```

```bash
cargo install trunk --locked
```

Then, from `crates/calc-app`:

```bash
trunk serve
```

That serves the calculator at http://localhost:8765 and rebuilds when files change. `trunk build` writes a static site to `crates/calc-app/dist/` that any static host can serve. The `.wasm` file is about 5 MB, roughly 2 MB once the server compresses it. In the browser, settings and memory are saved in the page's local storage.

The **Web** workflow (`.github/workflows/pages.yml`) publishes the site to GitHub Pages on every push to `main`. To turn it on, open the repository's *Settings → Pages* and set *Source* to *GitHub Actions*.

## Installers

| Platform | File | Installs to |
|---|---|---|
| Windows x64 / ARM64 | `scicalc82_<version>_x64-setup.exe`, `…_arm64-setup.exe` | `%LOCALAPPDATA%\SciCalc 82`, with Start menu and desktop shortcuts. No administrator rights needed. |
| macOS (Apple silicon / Intel) | `scicalc82_<version>_aarch64.dmg`, `…_x64.dmg` | Drag *SciCalc 82* to Applications |
| Debian, Ubuntu | `scicalc82_<version>_amd64.deb` (or `arm64`) | `sudo apt install ./scicalc82_*.deb` |
| Fedora, RHEL | `scicalc82-<version>-1.x86_64.rpm` (or `aarch64`) | `sudo dnf install ./scicalc82-*.rpm` |

### With GitHub Actions

The **Installers** workflow (`.github/workflows/release.yml`) builds all of them on GitHub's Windows, macOS and Linux machines:

- Run it from the *Actions* tab to download the installers as build artifacts.
- Push a tag such as `v0.1.0` to also attach them to a draft GitHub release. Publish the draft to make it the latest release.

The Linux packages are built on Ubuntu 22.04, so they also run on Debian 12, Fedora 36 and later, and RHEL 10.

### On your own machine

Each installer is built on its own operating system with [cargo-packager](https://github.com/crabnebula-dev/cargo-packager), plus [cargo-generate-rpm](https://github.com/cat-in-136/cargo-generate-rpm) for the .rpm:

```bash
cargo install cargo-packager cargo-generate-rpm --locked
```

**Windows**, for x64. Use `aarch64-pc-windows-msvc` for ARM64, which needs Visual Studio's *MSVC ARM64 build tools* component.

```bash
cargo build --release -p calc-app --target x86_64-pc-windows-msvc
```

```bash
cargo packager -p scicalc82 --release --target x86_64-pc-windows-msvc --formats nsis
```

**Linux** (or WSL), for the .deb and .rpm:

```bash
cargo build --release -p calc-app
```

```bash
cargo packager -p scicalc82 --release --formats deb
```

```bash
cargo generate-rpm -p crates/calc-app
```

**macOS:** `cargo packager -p scicalc82 --release --formats app` builds `SciCalc 82.app`. The workflow then signs it ad hoc and wraps it in a .dmg.

The Windows installer is written next to the executable in `target/<target>/release/`, the .deb to `target/release/` and the .rpm to `target/generate-rpm/`.

### Signing

The installers are not code-signed, so:

- **Windows:** SmartScreen may say it "protected your PC". Choose *More info*, then *Run anyway*.
- **macOS:** the first time, right-click the app and choose *Open*, or allow it in *System Settings → Privacy & Security*.

### Settings and icons

The package settings are in `crates/calc-app/Cargo.toml`, and the Linux `.desktop` file is in `packaging/linux/`. The icons in `crates/calc-app/icons/` are drawn by the same code as the window icon (`crates/calc-app/src/icon_pixels.rs`). Regenerate them after changing it:

```bash
cargo test -p calc-app write_package_icons -- --ignored
```

## Tests and screenshots

```bash
cargo test
```

This runs the unit, property and keystroke tests for the engine, plus layout tests for the display. The **CI** workflow runs them, along with `cargo fmt` and `cargo clippy`, on Windows, macOS and Linux for every push and pull request.

The images in the docs are rendered offscreen, without opening a window. Regenerate them after changing the UI:

| Command | Writes |
|---|---|
| `cargo test -p calc-app render_readme_images -- --ignored` | The README's `hero.png` and `demo.gif` |
| `cargo test -p calc-app render_manual_images -- --ignored` | The User Manual's screenshots in `docs/images/` |
| `cargo test -p calc-app render_screens -- --ignored` | Every screen, in `target/screens/`, for checking changes |

## Project layout

```
crates/calc-core   the engine, with no UI code: numbers, input editor, parser, evaluator,
                   menus and every mode, driven by key presses
crates/calc-app    the eframe/egui app: calculator body, keypad, dot-matrix LCD and
                   natural-display renderer; also the web build
packaging/linux    the .desktop file for the Linux packages
docs               the User Manual, this page and the design plan
.github/workflows  CI, the web site and the installers
```

`calc-core` has one entry point: `Calculator::press(Key)` changes the state, and the display reads it back. Because the engine doesn't depend on the UI, most behavior is tested through keystroke sequences such as `"mode 7 1 1 1 = 2 = 3 ="`.

The design decisions and milestones are in [PLAN.md](PLAN.md).
