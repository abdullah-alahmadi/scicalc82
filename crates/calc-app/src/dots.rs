//! A dot-matrix typeface for the LCD, drawn as individual dots.
//!
//! Glyphs sit on a 5×7 grid like the device's (punctuation is narrower),
//! with two extra rows below the baseline for g, j, p, q, y and the comma.
//! Characters without a glyph fall back to the regular LCD font.

use std::collections::HashMap;
use std::sync::LazyLock;

use eframe::egui::{Align, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, pos2, vec2};

/// Dot spacing as a fraction of the font size: 7 rows match the cap height
/// the layout engine assumes (about 0.73 em).
const PITCH: f32 = 0.105;
/// Below this many pixels per dot, dots touch (a crisp pixel font); from
/// here up they get a one-pixel gap, like an LCD's dot grid.
const GAP_FROM_PIXELS: f32 = 3.0;
/// Rows above the baseline.
const CAP_ROWS: usize = 7;

/// Each glyph is drawn as rows of `#` (lit) and `.` (unlit), top to bottom:
/// 7 rows, or 9 with descenders. All rows of a glyph have the same width.
#[rustfmt::skip]
const GLYPHS: &[(char, &[&str])] = &[
    (' ', &["...", "...", "...", "...", "...", "...", "..."]),
    ('0', &[".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."]),
    ('1', &["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."]),
    ('2', &[".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"]),
    ('3', &["#####", "...#.", "..#..", "...#.", "....#", "#...#", ".###."]),
    ('4', &["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."]),
    ('5', &["#####", "#....", "####.", "....#", "....#", "#...#", ".###."]),
    ('6', &["..##.", ".#...", "#....", "####.", "#...#", "#...#", ".###."]),
    ('7', &["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."]),
    ('8', &[".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."]),
    ('9', &[".###.", "#...#", "#...#", ".####", "....#", "...#.", ".##.."]),
    ('A', &[".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('B', &["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."]),
    ('C', &[".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."]),
    ('D', &["###..", "#..#.", "#...#", "#...#", "#...#", "#..#.", "###.."]),
    ('E', &["#####", "#....", "#....", "####.", "#....", "#....", "#####"]),
    ('F', &["#####", "#....", "#....", "####.", "#....", "#....", "#...."]),
    ('G', &[".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".####"]),
    ('H', &["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"]),
    ('I', &["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"]),
    ('J', &["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."]),
    ('K', &["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"]),
    ('L', &["#....", "#....", "#....", "#....", "#....", "#....", "#####"]),
    ('M', &["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"]),
    ('N', &["#...#", "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#"]),
    ('O', &[".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('P', &["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('Q', &[".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"]),
    ('R', &["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"]),
    ('S', &[".####", "#....", "#....", ".###.", "....#", "....#", "####."]),
    ('T', &["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."]),
    ('U', &["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]),
    ('V', &["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('W', &["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "#.#.#", ".#.#."]),
    ('X', &["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"]),
    ('Y', &["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."]),
    ('Z', &["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
    ('a', &[".....", ".....", ".###.", "....#", ".####", "#...#", ".####"]),
    ('b', &["#....", "#....", "#.##.", "##..#", "#...#", "#...#", "####."]),
    ('c', &[".....", ".....", ".###.", "#....", "#....", "#...#", ".###."]),
    ('d', &["....#", "....#", ".##.#", "#..##", "#...#", "#...#", ".####"]),
    ('e', &[".....", ".....", ".###.", "#...#", "#####", "#....", ".###."]),
    ('f', &["..##", ".#..", ".#..", "###.", ".#..", ".#..", ".#.."]),
    ('g', &[".....", ".....", ".####", "#...#", "#...#", ".####", "....#", "#...#", ".###."]),
    ('h', &["#....", "#....", "#.##.", "##..#", "#...#", "#...#", "#...#"]),
    ('i', &[".#.", "...", "##.", ".#.", ".#.", ".#.", "###"]),
    ('j', &["..#", "...", ".##", "..#", "..#", "..#", "..#", "#.#", ".#."]),
    ('k', &["#...", "#...", "#..#", "#.#.", "##..", "#.#.", "#..#"]),
    ('l', &["##.", ".#.", ".#.", ".#.", ".#.", ".#.", "###"]),
    ('m', &[".....", ".....", "##.#.", "#.#.#", "#.#.#", "#.#.#", "#.#.#"]),
    ('n', &[".....", ".....", "#.##.", "##..#", "#...#", "#...#", "#...#"]),
    ('o', &[".....", ".....", ".###.", "#...#", "#...#", "#...#", ".###."]),
    ('p', &[".....", ".....", "####.", "#...#", "#...#", "####.", "#....", "#....", "#...."]),
    ('q', &[".....", ".....", ".####", "#...#", "#...#", ".####", "....#", "....#", "....#"]),
    ('r', &[".....", ".....", "#.##.", "##..#", "#....", "#....", "#...."]),
    ('s', &[".....", ".....", ".####", "#....", ".###.", "....#", "####."]),
    ('t', &[".#..", ".#..", "###.", ".#..", ".#..", ".#.#", "..#."]),
    ('u', &[".....", ".....", "#...#", "#...#", "#...#", "#..##", ".##.#"]),
    ('v', &[".....", ".....", "#...#", "#...#", "#...#", ".#.#.", "..#.."]),
    ('w', &[".....", ".....", "#...#", "#...#", "#.#.#", "#.#.#", ".#.#."]),
    ('x', &[".....", ".....", "#...#", ".#.#.", "..#..", ".#.#.", "#...#"]),
    ('y', &[".....", ".....", "#...#", "#...#", "#...#", ".####", "....#", "#...#", ".###."]),
    ('z', &[".....", ".....", "#####", "...#.", "..#..", ".#...", "#####"]),
    ('.', &[".", ".", ".", ".", ".", ".", "#"]),
    (',', &["..", "..", "..", "..", "..", "..", ".#", ".#", "#."]),
    (':', &[".", ".", "#", ".", ".", "#", "."]),
    ('!', &["#", "#", "#", "#", "#", ".", "#"]),
    ('?', &[".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."]),
    ('+', &[".....", "..#..", "..#..", "#####", "..#..", "..#..", "....."]),
    ('-', &["...", "...", "...", "###", "...", "...", "..."]),
    ('−', &[".....", ".....", ".....", "#####", ".....", ".....", "....."]),
    ('×', &[".....", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "....."]),
    ('÷', &[".....", "..#..", ".....", "#####", ".....", "..#..", "....."]),
    ('*', &[".....", "..#..", "#.#.#", ".###.", "#.#.#", "..#..", "....."]),
    ('/', &["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."]),
    ('=', &[".....", ".....", "#####", ".....", "#####", ".....", "....."]),
    ('<', &["...#", "..#.", ".#..", "#...", ".#..", "..#.", "...#"]),
    ('>', &["#...", ".#..", "..#.", "...#", "..#.", ".#..", "#..."]),
    ('(', &["..#", ".#.", "#..", "#..", "#..", ".#.", "..#"]),
    (')', &["#..", ".#.", "..#", "..#", "..#", ".#.", "#.."]),
    ('[', &["###", "#..", "#..", "#..", "#..", "#..", "###"]),
    (']', &["###", "..#", "..#", "..#", "..#", "..#", "###"]),
    ('|', &["#", "#", "#", "#", "#", "#", "#"]),
    ('_', &[".....", ".....", ".....", ".....", ".....", ".....", "#####"]),
    ('%', &["##...", "##..#", "...#.", "..#..", ".#...", "#..##", "...##"]),
    ('#', &[".#.#.", ".#.#.", "#####", ".#.#.", "#####", ".#.#.", ".#.#."]),
    ('~', &[".....", ".....", ".#...", "#.#.#", "...#.", ".....", "....."]),
    ('^', &["..#..", ".#.#.", "#...#", ".....", ".....", ".....", "....."]),
    ('\'', &["#", "#", ".", ".", ".", ".", "."]),
    ('"', &["#.#", "#.#", "...", "...", "...", "...", "..."]),
    ('°', &[".#.", "#.#", ".#.", "...", "...", "...", "..."]),
    ('′', &[".#", ".#", "#.", "..", "..", "..", ".."]),
    ('″', &[".#.#", ".#.#", "#.#.", "....", "....", "....", "...."]),
    ('²', &["##.", "..#", ".#.", "###", "...", "...", "..."]),
    ('³', &["##.", "..#", ".#.", "..#", "##.", "...", "..."]),
    ('¹', &[".#.", "##.", ".#.", "###", "...", "...", "..."]),
    ('⁻', &["...", "###", "...", "...", "...", "...", "..."]),
    ('π', &[".....", "#####", ".#.#.", ".#.#.", ".#.#.", ".#.#.", ".#.#."]),
    ('θ', &[".###.", "#...#", "#...#", "#####", "#...#", "#...#", ".###."]),
    ('σ', &[".....", ".....", ".####", "#..#.", "#...#", "#...#", ".###."]),
    ('Σ', &["#####", "#....", ".#...", "..#..", ".#...", "#....", "#####"]),
    ('√', &["....#", "...#.", "...#.", "#..#.", ".#.#.", ".##..", "..#.."]),
    ('▲', &[".....", ".....", "..#..", ".###.", "#####", ".....", "....."]),
    ('▼', &[".....", ".....", "#####", ".###.", "..#..", ".....", "....."]),
    ('◀', &["...#", "..##", ".###", "####", ".###", "..##", "...#"]),
    ('▶', &["#...", "##..", "###.", "####", "###.", "##..", "#..."]),
    ('▸', &["...", "#..", "##.", "###", "##.", "#..", "..."]),
    ('∠', &[".....", "....#", "...#.", "..#..", ".#...", "#....", "#####"]),
];

/// A parsed glyph: `rows[r]` has bit `c` set when column `c` is lit.
pub struct Glyph {
    pub width: usize,
    pub rows: Vec<u8>,
}

static FONT: LazyLock<HashMap<char, Glyph>> = LazyLock::new(|| {
    GLYPHS
        .iter()
        .map(|&(c, rows)| {
            let width = rows[0].len();
            let rows = rows
                .iter()
                .map(|row| row.bytes().enumerate().fold(0, |bits, (i, b)| bits | u8::from(b == b'#') << i))
                .collect();
            (c, Glyph { width, rows })
        })
        .collect()
});

pub fn glyph(c: char) -> Option<&'static Glyph> {
    FONT.get(&c)
}

/// Fallback font for characters without a dot glyph.
fn fallback(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Dot spacing in points, snapped to whole pixels so dots stay crisp.
fn pitch(painter: &Painter, size: f32) -> f32 {
    let ppp = painter.pixels_per_point();
    (PITCH * size * ppp).round().max(1.0) / ppp
}

fn snap(painter: &Painter, v: f32) -> f32 {
    let ppp = painter.pixels_per_point();
    (v * ppp).round() / ppp
}

/// Horizontal advance of `c` at `size` (one dot of spacing after each glyph).
pub fn advance(painter: &Painter, c: char, size: f32) -> f32 {
    match glyph(c) {
        Some(g) => (g.width + 1) as f32 * pitch(painter, size),
        None => painter.layout_no_wrap(c.to_string(), fallback(size), Color32::BLACK).size().x,
    }
}

pub fn width(painter: &Painter, text: &str, size: f32) -> f32 {
    text.chars().map(|c| advance(painter, c, size)).sum()
}

/// Draws `text` with its baseline starting at `origin` (screen points).
pub fn paint(painter: &Painter, origin: Pos2, text: &str, size: f32, color: Color32) {
    let pitch = pitch(painter, size);
    let one_pixel = 1.0 / painter.pixels_per_point();
    let side = if pitch * painter.pixels_per_point() >= GAP_FROM_PIXELS { pitch - one_pixel } else { pitch };
    let dot = vec2(side, side);
    let origin = pos2(snap(painter, origin.x), snap(painter, origin.y));
    // All dots go into one mesh: no anti-aliased edges, which would fade
    // one-pixel dots away, and one shape instead of hundreds.
    let mut mesh = Mesh::default();
    let mut x = origin.x;
    for c in text.chars() {
        match glyph(c) {
            Some(g) => {
                for (r, bits) in g.rows.iter().enumerate() {
                    let top = origin.y + (r as f32 - CAP_ROWS as f32) * pitch;
                    for col in (0..g.width).filter(|col| bits & (1 << col) != 0) {
                        mesh.add_colored_rect(Rect::from_min_size(pos2(x + col as f32 * pitch, top), dot), color);
                    }
                }
            }
            None => {
                // Align the fallback glyph's baseline with the dots.
                let galley = painter.layout_no_wrap(c.to_string(), fallback(size), color);
                let baseline = galley
                    .rows
                    .first()
                    .and_then(|row| row.row.glyphs.first().map(|g| row.pos.y + g.pos.y))
                    .unwrap_or(galley.size().y * 0.8);
                painter.galley(pos2(x, origin.y - baseline), galley, color);
            }
        }
        x += advance(painter, c, size);
    }
    painter.add(Shape::mesh(mesh));
}

/// Draws `text` anchored at `pos` like `Painter::text`, aligning vertically
/// on the capital-letter height rather than the font's line box.
pub fn paint_aligned(painter: &Painter, pos: Pos2, align: Align2, text: &str, size: f32, color: Color32) {
    let w = width(painter, text, size);
    let cap = CAP_ROWS as f32 * pitch(painter, size);
    let x = match align.x() {
        Align::Min => pos.x,
        Align::Center => pos.x - w / 2.0,
        Align::Max => pos.x - w,
    };
    let baseline = match align.y() {
        Align::Min => pos.y + cap,
        Align::Center => pos.y + cap / 2.0,
        Align::Max => pos.y,
    };
    paint(painter, pos2(x, baseline), text, size, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_table_is_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for &(c, rows) in GLYPHS {
            assert!(seen.insert(c), "{c:?} defined twice");
            assert!(rows.len() == 7 || rows.len() == 9, "{c:?} has {} rows", rows.len());
            let width = rows[0].len();
            assert!((1..=5).contains(&width), "{c:?} is {width} dots wide");
            for row in rows {
                assert_eq!(row.len(), width, "{c:?} has uneven rows");
                assert!(row.bytes().all(|b| b == b'#' || b == b'.'), "{c:?} has a stray character");
            }
            // Descender rows are only for glyphs that need them.
            if rows.len() == 9 {
                assert!(rows[7..].iter().any(|r| r.contains('#')), "{c:?} has empty descender rows");
            }
        }
    }

    #[test]
    fn covers_the_lcd_text() {
        let samples = [
            "Math ERROR",
            "Syntax ERROR",
            "Stack ERROR",
            "Argument ERROR",
            "Insufficient MEM",
            "[AC]  :Cancel",
            "[◀][▶]:Goto",
            "S A M STO RCL hyp D R G FIX SCI STAT Math ▲ ▼",
            "1:COMP 2:STAT 3:TABLE 1:MthIO 2:LineIO 3:Deg 4:Rad 5:Gra 6:Fix 7:Sci 8:Norm",
            "1:ab/c 2:d/c Frequency? 1:ON 2:OFF Clear? 1:Setup 2:Memory 3:All DRG▸ 1:° 2:r 3:g",
            "Fix 0~9? Sci 0~9? Norm 1~2?",
            "1:Type 2:Data 3:Sum 4:Var 5:Reg 6:MinMax Σx² Σx Σy² Σy Σxy n σx sx σy sy minX maxX minY maxY",
            "sin( cos( tan( log( ln( Pol( Rec( RanInt#( Rnd( Abs( √( Ans Ran# π e ×10 − - × ÷ ° ′ ″ ! % , ( )",
            "r= θ= X= Y= f(X)= Table Range Start? End? Step? F(X) ERROR 1234567890.",
            "MAT 5:MATRIX Matrix? 1:MatA 2:MatB 3:MatC m×n? 1:3×3 1:Dim 2:Data 6:MatAns 7:det 8:Trn Dimension ERROR det( Trn(",
            "CMPLX 2:CMPLX 1:arg 2:Conjg 3:▸r∠θ 4:▸a+bi Complex Result? 1:a+bi 2:r∠θ 1+2i √2∠45 arg( Conjg(",
        ];
        for text in samples {
            let missing: String = text.chars().filter(|&c| glyph(c).is_none()).collect();
            assert!(missing.is_empty(), "no dot glyph for {missing:?} in {text:?}");
        }
    }
}
