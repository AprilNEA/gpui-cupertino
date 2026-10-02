use std::ops::Range;

use gpui::SharedString;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Default)]
struct Snapshot {
    text: SharedString,
    selection: Range<usize>,
    reversed: bool,
}

#[derive(Default)]
pub(super) struct Editing {
    pub text: SharedString,
    pub selection: Range<usize>,
    pub reversed: bool,
    pub marked: Option<Range<usize>>,
    composition_start: Option<Snapshot>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Editing {
    pub fn new(text: &str) -> Self {
        let text = single_line(text);
        let end = text.len();
        Self {
            text: text.into(),
            selection: end..end,
            ..Self::default()
        }
    }

    pub fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub fn select_to(&mut self, offset: usize, extend: bool) {
        self.unmark();
        let anchor = if !extend {
            offset
        } else if self.reversed {
            self.selection.end
        } else {
            self.selection.start
        };
        self.selection = anchor.min(offset)..anchor.max(offset);
        self.reversed = offset < anchor;
    }

    pub fn previous(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    pub fn next(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.text.len())
    }

    pub fn word_at(&self, offset: usize) -> Range<usize> {
        let offset = offset.min(self.text.len().saturating_sub(1));
        self.text
            .split_word_bound_indices()
            .find_map(|(start, word)| {
                (start + word.len() > offset).then_some(start..start + word.len())
            })
            .unwrap_or(self.text.len()..self.text.len())
    }

    pub fn to_utf16(&self, range: Range<usize>) -> Range<usize> {
        self.text[..range.start].encode_utf16().count()
            ..self.text[..range.end].encode_utf16().count()
    }

    pub fn byte_range(&self, range: Range<usize>) -> Range<usize> {
        byte_range(&self.text, range)
    }

    pub fn replace(&mut self, range_utf16: Option<Range<usize>>, text: &str) {
        let range = self.replacement_range(range_utf16);
        let before = self
            .composition_start
            .take()
            .unwrap_or_else(|| self.snapshot());
        let text = single_line(text);
        self.replace_bytes(range.clone(), &text);
        let end = range.start + text.len();
        self.selection = end..end;
        self.reversed = false;
        self.marked = None;
        self.record(before);
    }

    pub fn mark(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        selection_utf16: Option<Range<usize>>,
    ) {
        let range = self.replacement_range(range_utf16);
        if self.composition_start.is_none() {
            self.composition_start = Some(self.snapshot());
        }
        let text = single_line(text);
        self.replace_bytes(range.clone(), &text);
        let end = range.start + text.len();
        self.marked = (!text.is_empty()).then_some(range.start..end);
        // The IME selection is relative to the inserted text, not the document.
        self.selection = selection_utf16
            .map(|selection| byte_range(&text, selection))
            .map(|selection| range.start + selection.start..range.start + selection.end)
            .unwrap_or(end..end);
        self.reversed = false;
    }

    pub fn unmark(&mut self) {
        self.marked = None;
        if let Some(before) = self.composition_start.take() {
            self.record(before);
        }
    }

    pub fn undo(&mut self) {
        self.unmark();
        if let Some(before) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(before);
        }
    }

    pub fn redo(&mut self) {
        self.unmark();
        if let Some(after) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(after);
        }
    }

    fn replacement_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        range_utf16
            .map(|range| self.byte_range(range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone())
    }

    fn replace_bytes(&mut self, range: Range<usize>, text: &str) {
        let mut value = self.text.to_string();
        value.replace_range(range, text);
        self.text = value.into();
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            selection: self.selection.clone(),
            reversed: self.reversed,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.selection = snapshot.selection;
        self.reversed = snapshot.reversed;
    }

    fn record(&mut self, before: Snapshot) {
        if before.text != self.text {
            // ponytail: Keep 100 full-value snapshots for short fields; use edit deltas for documents.
            if self.undo.len() == 100 {
                self.undo.remove(0);
            }
            self.undo.push(before);
            self.redo.clear();
        }
    }
}

fn single_line(text: &str) -> String {
    text.replace(['\r', '\n', '\t', '\u{0085}', '\u{2028}', '\u{2029}'], " ")
}

fn byte_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = byte_offset(text, range.start, range.is_empty());
    let end = byte_offset(text, range.end, true).max(start);
    start..end
}

fn byte_offset(text: &str, offset: usize, round_up: bool) -> usize {
    let mut utf16 = 0;
    for (byte, character) in text.char_indices() {
        if utf16 == offset {
            return byte;
        }
        utf16 += character.len_utf16();
        if utf16 > offset {
            return if round_up {
                byte + character.len_utf8()
            } else {
                byte
            };
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::Editing;

    #[test]
    fn double_click_at_text_end_selects_the_last_word() {
        let input = Editing::new("hello world");
        assert_eq!(input.word_at(input.text.len()), 6..11);
        let input = Editing::new("你好");
        assert_eq!(input.word_at(input.text.len()), 3..6);
        assert_eq!(Editing::new("").word_at(0), 0..0);
    }

    #[test]
    fn grapheme_edits_and_reversed_selection_survive_undo() {
        let mut input = Editing::new("a👩🏽‍💻e\u{301}");
        let end = input.text.len();
        input.select_to(input.previous(end), true);
        assert!(input.reversed);
        input.replace(None, "");
        assert_eq!(input.text, "a👩🏽‍💻");
        input.select_to(input.previous(input.cursor()), true);
        input.replace(None, "");
        assert_eq!(input.text, "a");
        input.undo();
        assert_eq!(input.text, "a👩🏽‍💻");
        assert_eq!(input.selection, 1..16);
        assert!(input.reversed);
        input.undo();
        assert_eq!(input.text, "a👩🏽‍💻e\u{301}");
        input.redo();
        assert_eq!(input.text, "a👩🏽‍💻");
    }

    #[test]
    fn ime_selection_uses_inserted_utf16_and_composition_is_one_undo() {
        let mut input = Editing::new("🙂尾");
        input.select_to(4, false);
        input.mark(None, "ni", Some(1..2));
        assert_eq!(input.text, "🙂ni尾");
        assert_eq!(input.to_utf16(input.selection.clone()), 3..4);
        input.mark(None, "你😀", Some(1..3));
        assert_eq!(input.text, "🙂你😀尾");
        assert_eq!(input.to_utf16(input.selection.clone()), 3..5);
        assert_eq!(input.marked, Some(4..11));
        input.replace(None, "你好");
        assert_eq!(input.text, "🙂你好尾");
        assert_eq!(input.to_utf16(input.selection.clone()), 4..4);
        assert!(input.marked.is_none());
        input.undo();
        assert_eq!(input.text, "🙂尾");
        input.redo();
        assert_eq!(input.text, "🙂你好尾");
    }

    #[test]
    fn utf16_ranges_clamp_without_splitting_surrogates_and_input_stays_single_line() {
        let mut input = Editing::new("a😀z");
        assert_eq!(input.byte_range(2..3), 1..5);
        assert_eq!(input.byte_range(2..2), 5..5);
        assert_eq!(input.byte_range(90..100), 6..6);
        input.replace(Some(1..3), "中\r\n文\t好\u{2028}");
        assert_eq!(input.text, "a中  文 好 z");
        input.undo();
        assert_eq!(input.text, "a😀z");
        input.replace(None, "!");
        input.redo();
        assert_eq!(input.text, "a😀z!");
    }
}
