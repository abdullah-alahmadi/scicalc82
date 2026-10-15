# SciCalc 82

A desktop scientific calculator written in Rust. It looks and behaves like a classic school scientific calculator: a textbook-style ("natural") display, exact answers such as `√2/2` and `3π/4`, and the same keys, menus and modes.

The name, look and code are original.

For how to use every key, mode and menu, see the **[User Manual](docs/USER_MANUAL.md)**.

## Features

- **Natural display:** stacked fractions, real radicals, raised exponents, and a cursor that moves through them. LineIO mode gives a one-line display instead.
- **Dot-matrix LCD:** display text is drawn dot by dot from a built-in 5×7 typeface. Dots snap to screen pixels so small text stays sharp.
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
  - **CMPLX:** complex numbers in rectangular (`a+bi`) and polar (`r∠θ`) form, with exact results such as `2∠30 = √3+i`. Supports + − × ÷, integer powers, Abs, arg, Conjg and conversion between the two forms.
  - **STAT:** 1-VAR statistics and A+BX linear regression, with a data editor and an optional FREQ column.
  - **TABLE:** values of f(X) over a range.
  - **EQN:** solves two or three simultaneous linear equations, and quadratic and cubic equations. Answers are exact where possible (`X²−2=0` gives `±√2`), and complex roots are included.
  - **MATRIX:** matrices up to 3×3 (MatA, MatB, MatC and MatAns) with exact entries. Supports + − ×, multiplying or dividing by a number, x², x³ and integer powers, the inverse (x⁻¹), det, Trn (transpose) and Abs.
  - **VECTOR:** 2D and 3D vectors (VctA, VctB, VctC and VctAns) with exact components. Supports + −, multiplying or dividing by a number, the dot product, the cross product, Abs (length, such as `√14`) and the angle between two vectors.
- **Persistence:** settings, memory, mode, STAT data, the TABLE setup, matrices, vectors and EQN coefficients are saved between launches. On Windows the file is `%APPDATA%\scicalc82\data\app.ron`. If the file is damaged, the app starts fresh instead of failing.

## Build and run

You need a recent stable Rust toolchain (edition 2024).

```bash
cargo run --release -p calc-app
```

The optimized executable is written to `target/release/calc-app.exe`.

### Web version

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

That serves the calculator at http://localhost:8765 and rebuilds when files change. `trunk build` writes a static site to `crates/calc-app/dist/`, which you can upload to any static host (GitHub Pages, Netlify, and so on). The `.wasm` file is about 5 MB, roughly 2 MB once the server compresses it. In the browser, settings and memory are saved in the page's local storage.

### Windows icon

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
| `i` `<` | i and ∠ (CMPLX mode) |
| Ctrl+C | Copy the result |

The modes work like the device:
- **MODE:** 1 for COMP, 2 for CMPLX, 3 for STAT, 4 for TABLE, 5 for MATRIX, 6 for EQN, 7 for VECTOR.
- **SHIFT MODE:** opens SETUP. ▼ shows its second page, which includes the CMPLX result format (a+bi or r∠θ).
- **CMPLX mode:**
  - The ENG key types `i`, and SHIFT (−) types `∠`. Angles use the current angle unit.
  - SHIFT 2 opens the CMPLX menu: arg, Conjg, ▸r∠θ and ▸a+bi. Put ▸r∠θ or ▸a+bi at the end of a calculation to show its result in that form.
  - √ of a negative number gives an imaginary result. Real-only functions such as sin and log reject complex numbers, as on the device.
- **STAT mode:** SHIFT 1 opens the STAT menu (Type, Data, Sum, Var, Reg, MinMax).
- **EQN mode:**
  - Choose the equation type: 1 `anX+bnY=cn`, 2 `anX+bnY+cnZ=dn`, 3 `aX²+bX+c=0`, 4 `aX³+bX²+cX+d=0`.
  - Type each coefficient followed by =; any expression works, such as `1/2` or `√3`. Press = with nothing typed to solve.
  - Solutions appear one at a time. ▲/▼ move between them, S⇔D toggles exact and decimal, and = after the last (or AC) returns to the coefficients.
  - Linear systems can show `No Solution` or `Infinite Solution`. A leading coefficient of 0 in a quadratic or cubic is a Math ERROR.
- **MATRIX mode:**
  - On entering the mode, choose a matrix and its size (1×1 to 3×3; ▼ shows the 1×n sizes), then type each cell followed by =. AC returns to calculations.
  - SHIFT 4 opens the MATRIX menu: Dim (resize a matrix), Data (edit its cells), MatA, MatB, MatC, MatAns, det and Trn.
  - Matrix results are drawn in brackets and become MatAns; results that are numbers, such as det, become Ans.
  - Mismatched sizes give a Dimension ERROR, and inverting a singular matrix gives a Math ERROR.
- **VECTOR mode:**
  - On entering the mode, choose a vector and its dimension (3D or 2D), then type each component followed by =. AC returns to calculations.
  - SHIFT 5 opens the VECTOR menu: Dim, Data, VctA, VctB, VctC, VctAns, Dot (`·`) and Angle.
  - × between two vectors is the cross product; two 2D vectors give (0, 0, z). Vector results become VctAns.
  - Mixing 2D and 3D vectors gives a Dimension ERROR.
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

The manual's screenshots in `docs/images/` are generated the same way. Regenerate them after changing the UI:

```bash
cargo test -p calc-app render_manual_images -- --ignored
```
