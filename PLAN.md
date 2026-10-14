# Scientific Calculator in Rust: Plan

A desktop app (with an optional web build) that looks and behaves like a classic school scientific calculator, with a natural textbook display and exact results.

## Decisions

| Area | Choice |
|---|---|
| Feature scope | A non-programmable school scientific calculator |
| Display | Natural textbook: stacked fractions, radicals, superscripts, 2D cursor editing |
| UI | `egui` / `eframe`: native window, with WebAssembly as an option later |
| Numbers | Exact rationals, surds and π-multiples, falling back to decimal; S⇔D toggles between them |

**Naming:** don't use any manufacturer's name or logo in the app's title, icon or skin. The app has its own brand, "SciCalc 82".

## Architecture

The core idea is that **the calculator engine knows nothing about the UI.** The UI sends `Key` presses and draws whatever state the engine exposes. That lets us test whole keystroke sequences without a window:

```rust
let mut c = Calculator::default();
c.press_all(&[Key::Sin, Key::D3, Key::D0, Key::Exe]);
assert_eq!(c.result_text(), "1/2");
```

```
Calculator/
├─ Cargo.toml                 # workspace
├─ crates/
│  ├─ calc-core/              # pure logic, no UI deps, heavily unit-tested
│  │  ├─ key.rs               # Key enum (every physical key + SHIFT/ALPHA variants)
│  │  ├─ calculator.rs        # top-level state machine: press(Key), modes, menus
│  │  ├─ input/               # structured natural-display input
│  │  │  ├─ node.rs           # Input tree: Char | Frac{num,den} | Pow{exp} | Sqrt{rad} | Root{idx,rad} | LogBase{base,arg} | Abs{..}
│  │  │  └─ editor.rs         # cursor, insert/delete, move between slots
│  │  ├─ parse.rs             # input tree -> AST, calculator precedence rules
│  │  ├─ ast.rs
│  │  ├─ value/               # number system: Value = Surd | Pi | Decimal
│  │  │  ├─ rational.rs       # i128-based fraction, overflow -> decimal
│  │  │  ├─ surd.rs           # exact sums of ≤2 terms (p/q)·√r (rationals included)
│  │  │  ├─ decimal.rs        # f64 + calculator-style 15-digit rounding, 1e±99 range
│  │  │  └─ intmath.rs        # gcd, squarefree split, exact integer roots
│  │  ├─ eval/                # evaluator, functions, exact trig table
│  │  ├─ format.rs            # Norm1/Norm2/Fix/Sci/Eng, 10-digit display, a b/c vs d/c, °′″
│  │  ├─ menu.rs              # MODE, SETUP, DRG, CLR and STAT menus as data
│  │  ├─ stats.rs             # STAT data, 1-VAR and A+BX statistics
│  │  ├─ table.rs             # TABLE mode range and rows
│  │  └─ state.rs             # Ans, M, A–F/X/Y, settings, history
│  └─ calc-app/               # eframe binary
│     ├─ main.rs
│     ├─ app.rs               # eframe::App, wires keys -> core, core -> drawing
│     ├─ math/                # natural display: display tree, box layout, egui painter
│     ├─ widgets/lcd.rs       # LCD panel, indicator row, blinking cursor, scrolling
│     ├─ widgets/keypad.rs    # body + keys with SHIFT (yellow) / ALPHA (red) legends
│     ├─ keymap.rs            # PC keyboard -> Key
│     └─ theme.rs             # colors, fonts, scale
```

**Crates:** `eframe`/`egui`, `thiserror`, `serde` + `ron` (settings/memory persistence), `directories`, `rand` (Ran#, RanInt). Use `num-integer` for gcd if you don't want to hand-roll it. Dev: `proptest`, `insta` (layout snapshots).

### Key design points

1. **The input is a tree, not a string.** Natural display editing means a fraction key inserts a template with empty numerator and denominator slots, and ←/→ moves the cursor into and out of slots. The editor works on this tree, and the parser walks it to build an AST.
2. **Two-level numbers.** Each operation first tries an exact result (rational, `a√b/c`, `aπ/b`). If the result can't be exact (for example `sin 1`, or an overflow), it becomes a decimal. Special angles (0, 30, 45, 60, 90 and their multiples, in all three angle units) map to exact trig values through a lookup table.
3. **Calculator quirks live in one place.** Precedence (implicit multiplication binds tighter than ×÷, so `1÷2π` = `1/(2π)`), how unary minus interacts with `²` (`-2²` = −4), postfix functions, and automatically closing open parentheses on `=`.
4. **Errors match the device:** `Math ERROR`, `Syntax ERROR`, `Stack ERROR`, `Argument ERROR`. After an error, ←/→ jumps the cursor to the error location.
5. **Display rules:** a 10-digit mantissa; Norm1 switches to scientific notation below 10⁻² and Norm2 below 10⁻⁹; Fix 0–9, Sci 1–10, and ENG stepping.

## Feature checklist (v1)

- **Arithmetic:** + − × ÷, (−), parentheses, implicit multiplication, Ans, %.
- **Powers and roots:** x², x³, x⁻¹, xʸ, √, ∛, ˣ√, 10ˣ, eˣ.
- **Logs:** log, ln, logₐb.
- **Trig:** sin cos tan, their inverses, hyp variants, DEG/RAD/GRA, DRG▸ conversion, ° ′ ″ sexagesimal input and display.
- **Fractions:** a b/c and d/c input, S⇔D, conversion between mixed and improper.
- **Combinatorics and misc:** x!, nPr, nCr, Pol(, Rec(, Abs, Ran#, RanInt#, π, e.
- **Memory:** M+, M−, STO/RCL to A–F, X, Y, CLR (Setup / Memory / All).
- **SETUP menu:** MthIO/LineIO, angle unit, Fix/Sci/Norm, ab/c vs d/c, STAT frequency on/off.
- **History replay:** ▲/▼ recall of previous calculations.
- **Modes:** COMP; STAT (1-variable, plus linear regression); TABLE (f(x) over a range).
- **Status bar indicators:** S, A, M, STO, RCL, STAT, D/R/G, FIX, SCI, Math, ▲▼.
- **Keyboard shortcuts:** digits, operators, Enter = `=`, Backspace = DEL, Esc = AC, plus mappings for SHIFT and ALPHA.
- **Persistence:** settings and memory survive restarts, like the real device.

Stretch goals for later: CALC/SOLVE, alternate skins, an advanced feature pack (equations, integrals, vectors). Complex numbers (CMPLX) and matrices (MATRIX) are done. (The WASM web build is done: see the README.)

## Milestones

| # | Milestone | Done when |
|---|---|---|
| 0 ✅ | Workspace scaffold, `cargo fmt`/`clippy`/`test` passing, `git init` | Empty window opens |
| 1 ✅ | **Number core:** rational, exact surd/π, decimal fallback, formatter | Unit and property tests pass for arithmetic and display formatting |
| 2 ✅ | **Input tree + editor:** templates, cursor movement, insert/DEL | Editor tests cover slot navigation |
| 3 ✅ | **Parser + evaluator:** precedence, functions, angle modes, exact trig, Ans, memory | Keystroke-sequence tests produce the expected device results |
| 4 ✅ | **UI shell:** calculator body, keypad, SHIFT/ALPHA, keyboard map, LCD with a linear preview | A usable calculator, drawn linearly |
| 5 ✅ | **Natural display renderer:** box layout, fraction bars, radicals, superscripts, cursor, scrolling | Display looks like MathIO |
| 6 ✅ | **SETUP, STAT, TABLE modes**, history replay, error screens | Feature checklist complete |
| 7 ✅ | Polish: persistence, scaling/HiDPI, app icon, release build (optional WASM) | v1.0 |

Milestones 1–3 are the riskiest and most valuable part. Building them as pure, well-tested library code first means the UI work afterwards is mostly drawing.

## Testing strategy

- **Golden keystroke tests:** a table of `(keys, expected display)` cases checked against a real school scientific calculator, or its manual's examples.
- **Property tests:** exact and decimal paths agree within 1e-12. Formatting round-trips correctly.
- **Snapshot tests:** layout box trees for natural display expressions (`insta`).
- **Offscreen UI renders:** `cargo test -p calc-app render_screens -- --ignored` writes PNGs of key screens to `target/screens/` (egui_kittest, no window or desktop needed).
- **Manual UI checks** against reference photos of the device.

## UI decisions (milestone 4)

- **Style:** a stylized, flat take on the device: a dark body, a greenish LCD, SHIFT legends in gold and ALPHA legends in red. Key shapes are simplified.
- **Window:** resizable. The calculator is laid out in a fixed 360×650 design space and scaled uniformly to fit the window, so it keeps its proportions.
- **Fonts:** egui's built-in Hack (monospace) for the LCD. It covers √ ∛ ⁻¹ π θ □ ▲ ⇔. Hack is also added as a fallback for key labels. The few glyphs neither font has (ˣ ʳ ᵍ) are drawn as raised text. A dot-matrix LCD font can be bundled later in polish.
- **Keyboard:** see `keymap.rs`. Hovering over the model name shows the shortcut list.
