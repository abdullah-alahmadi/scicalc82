//! Natural-display input: the expression tree and the cursor editor.

mod editor;
mod node;

pub use editor::{Cursor, Editor, MAX_DEPTH, MAX_ITEMS, SlotRef};
pub use node::{Func, Item, Row, Template, TemplateKind, Token, Var};
