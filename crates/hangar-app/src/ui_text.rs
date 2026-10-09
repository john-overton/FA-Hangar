//! Single-line text editing for the dialog value and the outliner filter:
//! caret, selection, word moves, mouse placement and the text clipboard.
use super::{text_width, view::Action, App, Canvas, Key, Style};
use crate::ui::theme::color as c;
use alloc::{
    format,
    string::{String, ToString},
};

/// Caret and selection anchor as byte offsets into the field's text.
/// `END` follows the end of whatever the field holds, so a value set by
/// code keeps the caret after it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Caret {
    pub pos: usize,
    pub anchor: usize,
}
impl Caret {
    pub const END: Self = Self {
        pos: usize::MAX,
        anchor: usize::MAX,
    };
    pub const fn at(pos: usize) -> Self {
        Self { pos, anchor: pos }
    }
    /// Clamped to `text` and to character boundaries.
    pub fn clamp(self, text: &str) -> Self {
        let fit = |mut i: usize| {
            i = i.min(text.len());
            while !text.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        Self {
            pos: fit(self.pos),
            anchor: fit(self.anchor),
        }
    }
    /// The selected byte range, empty when nothing is selected.
    pub fn range(self, text: &str) -> (usize, usize) {
        let c = self.clamp(text);
        (c.pos.min(c.anchor), c.pos.max(c.anchor))
    }
}
/// Which field a key or click edits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Field {
    Prompt,
    Filter,
}
/// Per-App text state outside the prompt: the filter caret, the clipboard
/// used when the system one is unavailable, and a mouse selection in progress.
#[derive(Default)]
pub(super) struct TextState {
    pub filter: Option<Caret>,
    pub clip: String,
    pub drag: Option<(Field, [i32; 4])>,
}
/// Longest value a field takes.
pub(super) const MAX_LEN: usize = 1024;

fn word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}
fn prev(text: &str, i: usize) -> usize {
    text[..i].char_indices().next_back().map_or(0, |(j, _)| j)
}
fn next(text: &str, i: usize) -> usize {
    text[i..].chars().next().map_or(i, |ch| i + ch.len_utf8())
}
/// Start of the word before `i` (Ctrl+Left, Ctrl+Backspace).
fn word_left(text: &str, mut i: usize) -> usize {
    while i > 0 && !word(text[..i].chars().next_back().unwrap()) {
        i = prev(text, i);
    }
    while i > 0 && word(text[..i].chars().next_back().unwrap()) {
        i = prev(text, i);
    }
    i
}
/// End of the word after `i` (Ctrl+Right, Ctrl+Delete).
fn word_right(text: &str, mut i: usize) -> usize {
    while i < text.len() && !word(text[i..].chars().next().unwrap()) {
        i = next(text, i);
    }
    while i < text.len() && word(text[i..].chars().next().unwrap()) {
        i = next(text, i);
    }
    i
}
/// Printable ASCII from `s`, up to the first line break, tabs as spaces:
/// field values are single ASCII operands, names and paths.
fn sanitize(s: &str) -> (String, bool) {
    let line = s.split(['\r', '\n']).next().unwrap_or("");
    let mut dropped = line.len() != s.trim_end_matches(['\r', '\n']).len();
    let out = line
        .chars()
        .filter_map(|ch| match ch {
            '\t' => Some(' '),
            ' '..='~' => Some(ch),
            _ => {
                dropped = true;
                None
            }
        })
        .collect();
    (out, dropped)
}
/// Replace the selection with `s` (cut to the room left under `MAX_LEN`).
/// Returns whether `s` had to be cut.
fn insert(text: &mut String, caret: &mut Caret, s: &str) -> bool {
    let (a, b) = caret.range(text);
    let room = MAX_LEN.saturating_sub(text.len() - (b - a));
    let mut end = s.len().min(room);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    text.replace_range(a..b, &s[..end]);
    *caret = Caret::at(a + end);
    end < s.len()
}
/// What a key did to a field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Edit {
    Ignored,
    Moved,
    Changed,
}
/// Apply a caret, selection or editing key (not the clipboard) to `text`.
pub(super) fn apply(
    text: &mut String,
    caret: &mut Caret,
    key: Key,
    ctrl: bool,
    shift: bool,
) -> Edit {
    *caret = caret.clamp(text);
    let (a, b) = caret.range(text);
    let moved = |caret: &mut Caret, to: usize| {
        caret.pos = to;
        if !shift {
            caret.anchor = to;
        }
        Edit::Moved
    };
    match key {
        Key::Left if !shift && a != b => moved(caret, a),
        Key::Right if !shift && a != b => moved(caret, b),
        Key::Left => moved(
            caret,
            if ctrl {
                word_left(text, caret.pos)
            } else {
                prev(text, caret.pos)
            },
        ),
        Key::Right => moved(
            caret,
            if ctrl {
                word_right(text, caret.pos)
            } else {
                next(text, caret.pos)
            },
        ),
        Key::Home => moved(caret, 0),
        Key::End => moved(caret, text.len()),
        Key::Char('a') | Key::Char('A') if ctrl => {
            *caret = Caret {
                pos: text.len(),
                anchor: 0,
            };
            Edit::Moved
        }
        Key::Backspace | Key::Delete if a != b => {
            text.replace_range(a..b, "");
            *caret = Caret::at(a);
            Edit::Changed
        }
        Key::Backspace if a > 0 => {
            let from = if ctrl {
                word_left(text, a)
            } else {
                prev(text, a)
            };
            text.replace_range(from..a, "");
            *caret = Caret::at(from);
            Edit::Changed
        }
        Key::Delete if a < text.len() => {
            let to = if ctrl {
                word_right(text, a)
            } else {
                next(text, a)
            };
            text.replace_range(a..to, "");
            *caret = Caret::at(a);
            Edit::Changed
        }
        Key::Char(ch) if !ctrl && !ch.is_control() => {
            let mut buf = [0; 4];
            insert(text, caret, ch.encode_utf8(&mut buf));
            Edit::Changed
        }
        _ => Edit::Ignored,
    }
}
/// First byte shown when `text` is drawn `room` px wide with the caret at
/// `pos`: the start, or as far in as keeps the caret in view.
pub(super) fn first_visible(text: &str, pos: usize, room: i32, style: Style) -> usize {
    let mut start = 0;
    while start < pos && text_width(&text[start..pos], style) > room {
        start = next(text, start);
    }
    start
}
/// Byte offset in `text` nearest `dx` px right of the first visible byte.
pub(super) fn offset_at(text: &str, start: usize, dx: i32, style: Style) -> usize {
    let mut x = 0;
    for (i, ch) in text[start..].char_indices() {
        let w = super::char_width(ch, style);
        if dx < x + w / 2 {
            return start + i;
        }
        x += w;
    }
    text.len()
}
/// Draw `text` from `x` in a field `room` px wide whose box spans `top`..`top
/// + h`, with the selection fill and caret when `caret` is given.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    d: &mut Canvas,
    x: i32,
    base: i32,
    top: i32,
    h: i32,
    room: i32,
    text: &str,
    caret: Option<Caret>,
) {
    let style = Style::Value;
    let c0 = caret.unwrap_or(Caret::END).clamp(text);
    let start = first_visible(text, c0.pos, room, style);
    let mut end = text.len();
    while end > start && text_width(&text[start..end], style) > room {
        end = prev(text, end);
    }
    if let Some(caret) = caret {
        let caret = caret.clamp(text);
        let (a, b) = caret.range(text);
        let (a, b) = (a.clamp(start, end), b.clamp(start, end));
        if a < b {
            let sx = x + text_width(&text[start..a], style);
            d.rect(
                sx,
                top + 4,
                text_width(&text[a..b], style),
                h - 8,
                c::AMBER_DEEP,
            );
        }
    }
    d.styled(x, base, &text[start..end], c::INK, style);
    if caret.is_some() {
        let cx = x + text_width(&text[start..c0.pos], style);
        d.rect(cx, top + 4, 1, h - 8, c::INK);
    }
}
/// Text origin and room inside the dialog value box `rect`.
pub(super) fn prompt_area([x, _, w, _]: [i32; 4]) -> (i32, i32) {
    (x + 8, w - 20)
}
/// Text origin and room inside the outliner filter box `rect`.
pub(super) fn filter_area([x, _, w, _]: [i32; 4]) -> (i32, i32) {
    use crate::ui::theme::{metric as m, space};
    let tx = x + 5 + m::ICON_SM + space::SPACE_1;
    (tx, x + w - 5 - tx - 2)
}

impl App {
    fn field_parts(&mut self, field: Field) -> Option<(&mut String, &mut Caret)> {
        match field {
            Field::Prompt => self.prompt.as_mut().map(|p| (&mut p.value, &mut p.caret)),
            Field::Filter => {
                let caret = self.text.filter.get_or_insert(Caret::END);
                Some((&mut self.filter, caret))
            }
        }
    }
    /// A key for `field`: caret moves, selection, editing and Ctrl+C/X/V.
    pub(super) fn text_key(&mut self, field: Field, key: Key, ctrl: bool, shift: bool) -> Edit {
        let clip = match key {
            Key::Char(ch) if ctrl => ch.to_ascii_lowercase(),
            _ => '\0',
        };
        match clip {
            'c' | 'x' => {
                let Some((text, caret)) = self.field_parts(field) else {
                    return Edit::Ignored;
                };
                let (a, b) = caret.range(text);
                if a == b {
                    self.status = "Select text to copy; Ctrl+A selects all".into();
                    return Edit::Ignored;
                }
                let copied = text[a..b].to_string();
                let cut = clip == 'x';
                if cut {
                    text.replace_range(a..b, "");
                    *caret = Caret::at(a);
                }
                crate::platform::set_clipboard_text(&copied);
                self.status = format!(
                    "{} {} characters",
                    if cut { "Cut" } else { "Copied" },
                    copied.len()
                );
                self.text.clip = copied;
                if cut {
                    Edit::Changed
                } else {
                    Edit::Moved
                }
            }
            'v' => {
                let pasted =
                    crate::platform::clipboard_text().unwrap_or_else(|| self.text.clip.clone());
                let (clean, dropped) = sanitize(&pasted);
                let Some((text, caret)) = self.field_parts(field) else {
                    return Edit::Ignored;
                };
                if clean.is_empty() {
                    self.status = "Clipboard has no text for this field".into();
                    return Edit::Ignored;
                }
                let cut = insert(text, caret, &clean);
                self.status = if cut {
                    format!("Pasted up to the {MAX_LEN}-character limit")
                } else if dropped {
                    "Pasted the first line; line breaks and non-ASCII characters left out".into()
                } else {
                    format!("Pasted {} characters", clean.len())
                };
                Edit::Changed
            }
            _ => match self.field_parts(field) {
                Some((text, caret)) => apply(text, caret, key, ctrl, shift),
                None => Edit::Ignored,
            },
        }
    }
    /// A press on `field`'s box `rect` at `x`: place the caret (Shift
    /// extends the selection) and start a mouse selection.
    pub(super) fn text_press(&mut self, field: Field, rect: [i32; 4], x: i32, shift: bool) {
        let (origin, room) = match field {
            Field::Prompt => prompt_area(rect),
            Field::Filter => filter_area(rect),
        };
        let focused = field == Field::Prompt || self.filter_focus;
        let Some((text, caret)) = self.field_parts(field) else {
            return;
        };
        let current = if focused {
            caret.clamp(text)
        } else {
            Caret::END.clamp(text)
        };
        let start = first_visible(text, current.pos, room, Style::Value);
        let at = offset_at(text, start, x - origin, Style::Value);
        *caret = if shift {
            Caret {
                pos: at,
                anchor: current.anchor,
            }
        } else {
            Caret::at(at)
        };
        self.text.drag = Some((field, rect));
    }
    /// Pointer motion with the button held after `text_press`.
    pub(super) fn text_motion(&mut self, x: i32) {
        let Some((field, rect)) = self.text.drag else {
            return;
        };
        let (origin, room) = match field {
            Field::Prompt => prompt_area(rect),
            Field::Filter => filter_area(rect),
        };
        if let Some((text, caret)) = self.field_parts(field) {
            let c = caret.clamp(text);
            let start = first_visible(text, c.pos, room, Style::Value);
            let at = if x < origin {
                prev(text, start)
            } else {
                offset_at(text, start, x - origin, Style::Value)
            };
            caret.pos = at;
        }
    }
    /// Double-click in `field`: select the word under the caret.
    pub(super) fn text_word(&mut self, field: Field) {
        if let Some((text, caret)) = self.field_parts(field) {
            let at = caret.clamp(text).pos;
            let (a, b) = (word_left(text, next(text, at)), word_right(text, at));
            *caret = if a < b {
                Caret { pos: b, anchor: a }
            } else {
                Caret {
                    pos: text.len(),
                    anchor: 0,
                }
            };
        }
    }
    /// The field whose box is under (x, y) in the current layout.
    pub(super) fn text_field_at(&self, x: i32, y: i32) -> Option<(Field, [i32; 4])> {
        self.layout()
            .hits
            .into_iter()
            .rev()
            .find(|h| h.contains(x, y))
            .and_then(|h| match h.action {
                Action::PromptText => Some((Field::Prompt, h.rect)),
                Action::Filter => Some((Field::Filter, h.rect)),
                _ => None,
            })
    }
}

impl App {
    pub fn smoke_text_editing(&mut self) {
        let typed = |app: &mut App, s: &str| {
            for ch in s.chars() {
                app.key(Key::Char(ch), false, false);
            }
        };
        let value = |app: &App| app.prompt.as_ref().unwrap().value.clone();
        let caret = |app: &App| app.prompt.as_ref().unwrap().caret;
        self.demo();
        self.select_entry(self.doc.archive.find("DEMO.PT").unwrap());
        // A field prompt opens with the caret after the stored value.
        let f = self
            .brf
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .position(|f| f.kind == "string" && f.value == "\"DEMO.SH\"")
            .expect("demo PT names its shape");
        self.edit_field(f);
        let stored = value(self);
        assert_eq!(caret(self).clamp(&stored).pos, stored.len());
        // Home, arrows and typing insert at the caret, not at the end.
        self.prompt.as_mut().unwrap().value = "F14.SH".into();
        self.key(Key::Home, false, false);
        self.key(Key::Right, false, false);
        typed(self, "A");
        assert_eq!(value(self), "FA14.SH");
        self.key(Key::End, false, false);
        self.key(Key::Left, false, false);
        self.key(Key::Left, false, false);
        self.key(Key::Delete, false, false);
        assert_eq!(value(self), "FA14.H");
        self.key(Key::Backspace, false, false);
        assert_eq!(value(self), "FA14H");
        // Shift+arrows select; typing replaces the selection.
        self.key(Key::Home, false, false);
        self.key(Key::Right, false, true);
        self.key(Key::Right, false, true);
        typed(self, "X");
        assert_eq!(value(self), "X14H");
        // Ctrl+Left/Right move by word; Ctrl+Backspace deletes one.
        self.prompt.as_mut().unwrap().value = "ALPHA BRAVO.SH".into();
        self.prompt.as_mut().unwrap().caret = Caret::END;
        self.key(Key::Left, true, false);
        assert_eq!(caret(self).pos, "ALPHA BRAVO.".len());
        self.key(Key::Left, true, false);
        assert_eq!(caret(self).pos, "ALPHA ".len());
        self.key(Key::Backspace, true, false);
        assert_eq!(value(self), "BRAVO.SH");
        // Ctrl+A selects all (typing replaces), Ctrl+C/X/V use the clipboard.
        self.key(Key::Char('a'), true, false);
        self.key(Key::Char('c'), true, false);
        assert_eq!(self.text.clip, "BRAVO.SH");
        self.key(Key::End, false, false);
        self.key(Key::Char('v'), true, false);
        assert_eq!(value(self), "BRAVO.SHBRAVO.SH");
        self.key(Key::Char('a'), true, false);
        self.key(Key::Char('x'), true, false);
        assert_eq!(value(self), "");
        assert_eq!(self.text.clip, "BRAVO.SHBRAVO.SH");
        // Pasted text keeps its first line and printable ASCII only.
        self.text.clip = "\"F16.SH\"\r\nnext".into();
        self.key(Key::Char('v'), true, false);
        assert_eq!(value(self), "\"F16.SH\"");
        assert!(self.status.contains("line breaks"));
        // The value box is a hit: a click places the caret, a drag selects.
        let hit = self
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::PromptText))
            .expect("dialog value box");
        let (origin, _) = prompt_area(hit.rect);
        let y = hit.rect[1] + hit.rect[3] / 2;
        let after = |s: &str| origin + text_width(s, Style::Value);
        self.pointer(after("\"F1") + 1, y, 1, true, false);
        self.pointer(after("\"F1") + 1, y, 1, false, false);
        assert_eq!(caret(self), Caret::at(3));
        self.pointer(after("\"") + 1, y, 1, true, false);
        self.motion(after("\"F16.SH") + 1, y, false);
        self.pointer(after("\"F16.SH") + 1, y, 1, false, false);
        assert_eq!(caret(self).range(&value(self)), (1, 7));
        self.key(Key::Char('a'), true, false);
        typed(self, "\"F16.SH\"");
        // Backends send the press, then the double-click.
        self.pointer(after("\"F1") + 1, y, 1, true, false);
        self.double_click(after("\"F1") + 1, y);
        self.pointer(after("\"F1") + 1, y, 1, false, false);
        assert_eq!(caret(self).range(&value(self)), (1, 4), "word F16");
        // The edited value commits like a typed one.
        self.key(Key::Enter, false, false);
        assert!(self.prompt.is_none(), "{}", self.status);
        assert_eq!(
            self.brf.as_ref().unwrap().fields[f].value,
            "\"F16.SH\"",
            "{}",
            self.status
        );
        self.key(Key::Char('z'), true, false);
        assert_eq!(self.brf.as_ref().unwrap().fields[f].value, stored);
        // The filter edits in place too; Esc leaves the text.
        self.act(Action::Filter);
        typed(self, "DEMOPT");
        self.key(Key::Left, false, false);
        self.key(Key::Left, false, false);
        typed(self, ".");
        assert_eq!(self.filter, "DEMO.PT");
        self.key(Key::Home, false, true);
        self.key(Key::Char('c'), true, false);
        assert_eq!(self.text.clip, "DEMO.");
        self.key(Key::Escape, false, false);
        assert!(!self.filter_focus && self.filter == "DEMO.PT");
        self.filter.clear();
        *self.text = TextState::default();
    }
}
