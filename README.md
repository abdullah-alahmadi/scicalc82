# SciCalc 82

A desktop scientific calculator written in Rust. It looks and behaves like a classic school scientific calculator: a textbook-style ("natural") display, exact answers such as `√2/2` and `3π/4`, and the same keys, menus and modes.

The name, look and code are original.

## Features

- **Natural display:** stacked fractions, real radicals, raised exponents, and a cursor that moves through them. LineIO mode gives a one-line display instead.
- **Exact arithmetic:** fractions, sums of square roots (`1+√5`) and multiples of π stay exact. S⇔D switches to the decimal form.
  - Special angles are exact too: `sin 30° = 1/2`, `cos(π/4) = √2/2`, and `sin⁻¹(1/2)` gives `π/6` in radians.
  - Decimals carry 15 significant digits internally and show 10, with results limited to |x| < 10¹⁰⁰.
- **Functions:**
  - Trig and inverse trig, hyperbolic functions, logs (including log base n), powers and roots (including ˣ√).
  - x!, nPr, nCr, Pol/Rec, Abs, Ran#, RanInt#, Rnd, °′″ input and output, and DRG▸ angle conversion.
- **Memory and results:** Ans, A–F, X, Y and M; STO/RCL and M+/M−; history replay with ▲/▼.
  - Errors jump the cursor to where the problem is, like the device.
- **SETUP:** MthIO/LineIO, Deg/Rad/Gra, Fix/Sci/Norm, ab/c or d/c fractions, and STAT frequency on/off.
- **Modes:**
  - **COMP:** ordinary calculations.
  - **STAT:** 1-VAR statistics and A+BX linear regression, with a data editor and an optional FREQ column.
  - **TABLE:** values of f(X) over a range.
- **Persistence:** settings, memory, mode, STAT data and the TABLE setup are saved between launches. On Windows the file is `%APPDATA%\scicalc82\data\app.ron`. If the file is damaged, the app starts fresh instead of failing.

## Build and run

You need a recent stable Rust toolchain (edition 2024).

```bash
cargo run --release -p calc-app
```

The optimized executable is written to `target/release/calc-app.exe`.

On Windows, the build also embeds the app icon in the exe, so Explorer and shortcuts show it. It's generated from the same drawing code as the window icon (`crates/calc-app/src/icon_pixels.rs`). Embedding needs the resource compiler from the Windows SDK. If that's missing, the build still succeeds and prints a warning, and the exe simply has no icon.

## Using it

Click the keys, or use the PC keyboard. Hovering over the model name in the app shows this list too.

| Keys | Action |
|---|---|
| `0`–`9` `.` `+` `-` `*` `/` `(` `)` `^` | Same as the calculator keys |
| Enter, `=` | = |
| Backspace / Esc, Delete | DEL / AC |
| Arrow keys | Move the cursor, switch fraction slots, replay history |
| `s` `c` `t` / `S` `C` `T` | sin cos tan / their inverses |
| `h` `l` `n` `r` `q` | hyp, log, ln, √, x² |
| `f` `!` `%` `,` | Fraction, x!, %, comma |
| `p` `e` `E` | π, e, ×10ˣ |
| `a` `d` `m` | Ans, S⇔D, M+ |
| Ctrl+C | Copy the result |

The modes work like the device:
- **MODE:** 1 for COMP, 2 for STAT, 3 for TABLE.
- **SHIFT MODE:** opens SETUP. ▼ shows its second page.
- **STAT mode:** SHIFT 1 opens the STAT menu (Type, Data, Sum, Var, Reg, MinMax).
- **CLR (SHIFT 9):** resets setup, memory, or both.

## Project layout

```
crates/calc-core   the engine, with no UI code: numbers, input editor, parser, evaluator,
                   menus, STAT and TABLE, driven by key presses
crates/calc-app    the eframe/egui app: calculator body, keypad, LCD and natural-display renderer
PLAN.md            design decisions and milestones
```

`calc-core` exposes one entry point: `Calculator::press(Key)` changes the state, and the display reads it back. Because the engine doesn't depend on the UI, most behavior is tested through keystroke sequences.

## Tests

```bash
cargo test
```

This runs unit, property and keystroke tests for the engine, plus layout tests for the display.

To render the main screens to PNG files without opening a window:

```bash
cargo test -p calc-app render_screens -- --ignored
```

The images are written to `target/screens/`.
