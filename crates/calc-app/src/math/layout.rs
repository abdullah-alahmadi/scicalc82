//! Box layout for the natural display.
//!
//! Every laid-out piece has a width and extends `ascent` above and `descent`
//! below its baseline. Coordinates are relative to the left end of the
//! baseline, with y pointing down (so the area above the baseline is negative).
//! Sizes are in design units; the painter scales them.

use eframe::egui::{Pos2, Rect, pos2, vec2};

use super::nodes::Node;

// Proportions of the font size.
const ASCENT: f32 = 0.78;
const DESCENT: f32 = 0.24;
/// Scripts are this fraction of the surrounding size...
const SCRIPT_RATIO: f32 = 0.68;
/// ...but never smaller than this fraction of the base size.
const MIN_SCRIPT_RATIO: f32 = 0.5;
/// Height of the fraction bar above the baseline.
const AXIS: f32 = 0.3;
const FRACTION_GAP: f32 = 0.14;
const FRACTION_PAD: f32 = 0.12;
const LINE: f32 = 0.075;

/// Text measurement, so layout can be tested without a GUI.
pub trait Measure {
    fn width(&self, text: &str, size: f32) -> f32;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Text { pos: Pos2, text: String, size: f32 },
    Line { from: Pos2, to: Pos2, width: f32 },
    Path { points: Vec<Pos2>, width: f32 },
    SlotBox { rect: Rect, width: f32 },
}

/// Where the cursor bar goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorBar {
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Laid {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub prims: Vec<Prim>,
    pub cursor: Option<CursorBar>,
}

impl Laid {
    fn empty(size: f32) -> Self {
        Self { width: 0.0, ascent: ASCENT * size, descent: DESCENT * size, prims: Vec::new(), cursor: None }
    }

    /// Adds `child` with its baseline origin at (`x`, `y`), growing the extents.
    fn place(&mut self, child: Laid, x: f32, y: f32) {
        let offset = vec2(x, y);
        self.prims.extend(child.prims.into_iter().map(|p| match p {
            Prim::Text { pos, text, size } => Prim::Text { pos: pos + offset, text, size },
            Prim::Line { from, to, width } => Prim::Line { from: from + offset, to: to + offset, width },
            Prim::Path { points, width } => {
                Prim::Path { points: points.into_iter().map(|p| p + offset).collect(), width }
            }
            Prim::SlotBox { rect, width } => Prim::SlotBox { rect: rect.translate(offset), width },
        }));
        if let Some(c) = child.cursor {
            self.cursor = Some(CursorBar { x: c.x + x, top: c.top + y, bottom: c.bottom + y });
        }
        self.ascent = self.ascent.max(child.ascent - y);
        self.descent = self.descent.max(child.descent + y);
        self.width = self.width.max(x + child.width);
    }
}

struct Ctx<'a> {
    measure: &'a dyn Measure,
    base: f32,
}

impl Ctx<'_> {
    fn script(&self, size: f32) -> f32 {
        (size * SCRIPT_RATIO).max(self.base * MIN_SCRIPT_RATIO)
    }
}

/// Lays out a row of nodes at font size `size`.
pub fn layout(nodes: &[Node], size: f32, measure: &dyn Measure) -> Laid {
    row(&Ctx { measure, base: size }, nodes, size)
}

fn row(ctx: &Ctx<'_>, nodes: &[Node], size: f32) -> Laid {
    let mut out = Laid::empty(size);
    // Ascent of the previous item, so exponents clear tall bases like fractions.
    let mut previous_ascent = ASCENT * size;
    for node in nodes {
        match node {
            Node::Cursor => {
                out.cursor = Some(CursorBar { x: out.width, top: -ASCENT * size, bottom: DESCENT * size });
            }
            Node::Sup(children) => {
                let s = ctx.script(size);
                let script = row(ctx, children, s);
                let raise = (previous_ascent - script.ascent * 0.4).max(size * 0.42);
                let x = out.width + size * 0.03;
                out.place(script, x, -raise);
            }
            Node::Sub(children) => {
                let s = ctx.script(size);
                let script = row(ctx, children, s);
                let x = out.width + size * 0.03;
                out.place(script, x, size * 0.28);
            }
            _ => {
                let item = single(ctx, node, size);
                previous_ascent = item.ascent;
                let x = out.width;
                out.place(item, x, 0.0);
            }
        }
    }
    out
}

fn single(ctx: &Ctx<'_>, node: &Node, size: f32) -> Laid {
    let mut out = Laid::empty(size);
    match node {
        Node::Text(text) => {
            out.width = ctx.measure.width(text, size);
            out.prims.push(Prim::Text { pos: pos2(0.0, 0.0), text: text.clone(), size });
        }
        Node::Empty => {
            let (w, h, pad) = (0.55 * size, 0.7 * size, 0.08 * size);
            let rect = Rect::from_min_size(pos2(pad, -h - 0.02 * size), vec2(w, h));
            out.prims.push(Prim::SlotBox { rect, width: LINE * size * 0.8 });
            out.width = w + 2.0 * pad;
        }
        Node::Frac(num, den) => fraction(ctx, &mut out, num, den, size),
        Node::Root { index, radicand } => root(ctx, &mut out, index.as_deref(), radicand, size),
        Node::Abs(content) => {
            let inner = row(ctx, content, size);
            let (pad, gap) = (0.12 * size, 0.06 * size);
            let (top, bottom) = (-inner.ascent - gap, inner.descent + gap);
            let width = inner.width + 4.0 * pad;
            for x in [pad, width - pad] {
                out.prims.push(Prim::Line { from: pos2(x, top), to: pos2(x, bottom), width: LINE * size });
            }
            out.place(inner, 2.0 * pad, 0.0);
            out.width = width;
        }
        Node::Sup(_) | Node::Sub(_) | Node::Cursor => unreachable!("handled by row"),
    }
    out
}

fn fraction(ctx: &Ctx<'_>, out: &mut Laid, num: &[Node], den: &[Node], size: f32) {
    let num = row(ctx, num, size);
    let den = row(ctx, den, size);
    let (pad, gap, axis) = (FRACTION_PAD * size, FRACTION_GAP * size, AXIS * size);
    let bar = num.width.max(den.width) + 2.0 * pad;
    let margin = 0.06 * size;
    let num_y = -axis - gap - num.descent;
    let den_y = -axis + gap + den.ascent;
    let (num_x, den_x) = (margin + (bar - num.width) / 2.0, margin + (bar - den.width) / 2.0);
    out.prims.push(Prim::Line { from: pos2(margin, -axis), to: pos2(margin + bar, -axis), width: LINE * size });
    out.place(num, num_x, num_y);
    out.place(den, den_x, den_y);
    out.width = bar + 2.0 * margin;
}

fn root(ctx: &Ctx<'_>, out: &mut Laid, index: Option<&[Node]>, radicand: &[Node], size: f32) {
    let inner = row(ctx, radicand, size);
    let gap = 0.12 * size;
    let top = -inner.ascent - gap;
    let bottom = inner.descent;
    let tick_y = -0.32 * size;
    // An index sits above the tick; shift the sign right to make room for it.
    let mut sign_x = 0.04 * size;
    if let Some(index) = index {
        let index = row(ctx, index, ctx.script(size));
        let index_x = sign_x;
        sign_x = (index_x + index.width - 0.12 * size).max(sign_x);
        let index_y = tick_y - 0.12 * size - index.descent;
        out.place(index, index_x, index_y);
    }
    let sign_w = 0.5 * size;
    let points = vec![
        pos2(sign_x, tick_y + 0.04 * size),
        pos2(sign_x + 0.12 * size, tick_y - 0.04 * size),
        pos2(sign_x + 0.28 * size, bottom),
        pos2(sign_x + sign_w, top),
        pos2(sign_x + sign_w + inner.width + 0.1 * size, top),
    ];
    out.prims.push(Prim::Path { points, width: LINE * size });
    let content_x = sign_x + sign_w + 0.05 * size;
    let width = content_x + inner.width + 0.12 * size;
    out.place(inner, content_x, 0.0);
    out.ascent = out.ascent.max(-top + LINE * size);
    out.width = width;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A monospace stand-in: every character is 0.6 em wide.
    struct Mono;

    impl Measure for Mono {
        fn width(&self, text: &str, size: f32) -> f32 {
            text.chars().count() as f32 * 0.6 * size
        }
    }

    fn t(s: &str) -> Node {
        Node::Text(s.into())
    }

    fn texts(laid: &Laid) -> Vec<(String, Pos2, f32)> {
        laid.prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { pos, text, size } => Some((text.clone(), *pos, *size)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn text_runs_sit_on_one_baseline() {
        let laid = layout(&[t("12"), t("+"), t("3")], 10.0, &Mono);
        assert_eq!(laid.width, 24.0);
        assert!(texts(&laid).iter().all(|(_, pos, _)| pos.y == 0.0));
        assert_eq!(texts(&laid)[2].1.x, 18.0);
    }

    #[test]
    fn fraction_stacks_and_centers() {
        let laid = layout(&[Node::Frac(vec![t("1")], vec![t("234")])], 10.0, &Mono);
        let parts = texts(&laid);
        let (num, den) = (&parts[0], &parts[1]);
        assert!(num.1.y < -AXIS * 10.0, "numerator above the bar");
        assert!(den.1.y > 0.0, "denominator below the baseline");
        // Centered: the numerator's center matches the denominator's.
        let center = |x: f32, chars: f32| x + chars * 6.0 / 2.0;
        assert!((center(num.1.x, 1.0) - center(den.1.x, 3.0)).abs() < 1e-4);
        assert!(laid.ascent > ASCENT * 10.0 && laid.descent > DESCENT * 10.0);
        assert!(laid.prims.iter().any(|p| matches!(p, Prim::Line { .. })));
    }

    #[test]
    fn superscripts_are_raised_and_smaller() {
        let laid = layout(&[t("2"), Node::Sup(vec![t("10")])], 10.0, &Mono);
        let parts = texts(&laid);
        assert!(parts[1].1.y < 0.0);
        assert!(parts[1].2 < 10.0);
        // A power on a fraction rises above the fraction.
        let tall = layout(&[Node::Frac(vec![t("1")], vec![t("2")]), Node::Sup(vec![t("9")])], 10.0, &Mono);
        let y_of = |s: &str| texts(&tall).into_iter().find(|(text, ..)| text == s).unwrap().1.y;
        assert!(y_of("9") < y_of("1"), "exponent above the numerator's baseline");
    }

    #[test]
    fn nested_scripts_stop_shrinking() {
        let laid =
            layout(&[t("2"), Node::Sup(vec![t("3"), Node::Sup(vec![t("4"), Node::Sup(vec![t("5")])])])], 10.0, &Mono);
        let sizes: Vec<f32> = texts(&laid).iter().map(|(_, _, s)| *s).collect();
        assert_eq!(sizes[0], 10.0);
        assert!(sizes[1] < 10.0);
        assert_eq!(sizes[3], 10.0 * MIN_SCRIPT_RATIO);
    }

    #[test]
    fn radical_covers_its_content() {
        let laid = layout(&[Node::Root { index: None, radicand: vec![t("2")] }], 10.0, &Mono);
        let Some(Prim::Path { points, .. }) = laid.prims.iter().find(|p| matches!(p, Prim::Path { .. })) else {
            panic!("no radical sign");
        };
        let content_x = texts(&laid)[0].1.x;
        let overline_end = points.last().unwrap().x;
        assert!(overline_end >= content_x + 6.0, "overline spans the radicand");
        assert!(laid.ascent > ASCENT * 10.0);
        // An index pushes the sign to the right.
        let indexed = layout(&[Node::Root { index: Some(vec![t("10")]), radicand: vec![t("2")] }], 10.0, &Mono);
        let radicand_x = texts(&indexed).iter().find(|(s, ..)| s == "2").unwrap().1.x;
        assert!(radicand_x > content_x);
    }

    #[test]
    fn cursor_follows_its_slot() {
        let laid = layout(&[t("1"), Node::Frac(vec![t("2")], vec![Node::Cursor, Node::Empty])], 10.0, &Mono);
        let cursor = laid.cursor.expect("cursor recorded");
        assert!(cursor.top > 0.0 - AXIS * 10.0, "cursor is in the denominator");
        let in_exponent = layout(&[t("2"), Node::Sup(vec![t("3"), Node::Cursor])], 10.0, &Mono);
        let c = in_exponent.cursor.unwrap();
        assert!(c.bottom - c.top < 10.0, "cursor shrinks with the script");
        assert!(c.bottom < DESCENT * 10.0);
    }
}
