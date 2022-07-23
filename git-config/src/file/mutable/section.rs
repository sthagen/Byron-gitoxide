use std::{
    borrow::Cow,
    ops::{Deref, Range},
};

use bstr::{BStr, BString, ByteSlice, ByteVec};
use smallvec::SmallVec;

use crate::{
    file::{
        self,
        mutable::{escape_value, Whitespace},
        Index, Section, Size,
    },
    lookup, parse,
    parse::{section::Key, Event},
    value::{normalize, normalize_bstr, normalize_bstring},
};

/// A opaque type that represents a mutable reference to a section.
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct SectionMut<'a, 'event> {
    section: &'a mut Section<'event>,
    implicit_newline: bool,
    whitespace: Whitespace<'event>,
    newline: SmallVec<[u8; 2]>,
}

/// Mutating methods.
impl<'a, 'event> SectionMut<'a, 'event> {
    /// Adds an entry to the end of this section name `key` and `value`.
    pub fn push<'b>(&mut self, key: Key<'event>, value: impl Into<&'b BStr>) {
        let body = &mut self.section.body.0;
        if let Some(ws) = &self.whitespace.pre_key {
            body.push(Event::Whitespace(ws.clone()));
        }

        body.push(Event::SectionKey(key));
        body.extend(self.whitespace.key_value_separators());
        body.push(Event::Value(escape_value(value.into()).into()));
        if self.implicit_newline {
            body.push(Event::Newline(BString::from(self.newline.to_vec()).into()));
        }
    }

    /// Removes all events until a key value pair is removed. This will also
    /// remove the whitespace preceding the key value pair, if any is found.
    pub fn pop(&mut self) -> Option<(Key<'_>, Cow<'event, BStr>)> {
        let mut values = Vec::new();
        // events are popped in reverse order
        let body = &mut self.section.body.0;
        while let Some(e) = body.pop() {
            match e {
                Event::SectionKey(k) => {
                    // pop leading whitespace
                    if let Some(Event::Whitespace(_)) = body.last() {
                        body.pop();
                    }

                    if values.len() == 1 {
                        let value = values.pop().expect("vec is non-empty but popped to empty value");
                        return Some((k, normalize(value)));
                    }

                    return Some((
                        k,
                        normalize_bstring({
                            let mut s = BString::default();
                            for value in values.into_iter().rev() {
                                s.push_str(value.as_ref());
                            }
                            s
                        }),
                    ));
                }
                Event::Value(v) | Event::ValueNotDone(v) | Event::ValueDone(v) => values.push(v),
                _ => (),
            }
        }
        None
    }

    /// Sets the last key value pair if it exists, or adds the new value.
    /// Returns the previous value if it replaced a value, or None if it adds
    /// the value.
    pub fn set<'b>(&mut self, key: Key<'event>, value: impl Into<&'b BStr>) -> Option<Cow<'event, BStr>> {
        match self.key_and_value_range_by(&key) {
            None => {
                self.push(key, value);
                None
            }
            Some((_, value_range)) => {
                let range_start = value_range.start;
                let ret = self.remove_internal(value_range);
                self.section
                    .body
                    .0
                    .insert(range_start, Event::Value(escape_value(value.into()).into()));
                Some(ret)
            }
        }
    }

    /// Removes the latest value by key and returns it, if it exists.
    pub fn remove(&mut self, key: impl AsRef<str>) -> Option<Cow<'event, BStr>> {
        let key = Key::from_str_unchecked(key.as_ref());
        let (key_range, _value_range) = self.key_and_value_range_by(&key)?;
        Some(self.remove_internal(key_range))
    }

    /// Adds a new line event. Note that you don't need to call this unless
    /// you've disabled implicit newlines.
    pub fn push_newline(&mut self) {
        self.section
            .body
            .0
            .push(Event::Newline(Cow::Owned(BString::from(self.newline.to_vec()))));
    }

    /// Return the newline used when calling [`push_newline()`][Self::push_newline()].
    pub fn newline(&self) -> &BStr {
        self.newline.as_slice().as_bstr()
    }

    /// Enables or disables automatically adding newline events after adding
    /// a value. This is _enabled by default_.
    pub fn set_implicit_newline(&mut self, on: bool) {
        self.implicit_newline = on;
    }

    /// Sets the exact whitespace to use before each newly created key-value pair,
    /// with only whitespace characters being permissible.
    ///
    /// The default is 2 tabs.
    /// Set to `None` to disable adding whitespace before a key value.
    ///
    /// # Panics
    ///
    /// If non-whitespace characters are used. This makes the method only suitable for validated
    /// or known input.
    pub fn set_leading_whitespace(&mut self, whitespace: Option<Cow<'event, BStr>>) {
        assert!(
            whitespace
                .as_deref()
                .map_or(true, |ws| ws.iter().all(|b| b.is_ascii_whitespace())),
            "input whitespace must only contain whitespace characters."
        );
        self.whitespace.pre_key = whitespace;
    }

    /// Returns the whitespace this section will insert before the
    /// beginning of a key, if any.
    #[must_use]
    pub fn leading_whitespace(&self) -> Option<&BStr> {
        self.whitespace.pre_key.as_deref()
    }

    /// Returns the whitespace to be used before and after the `=` between the key
    /// and the value.
    ///
    /// For example, `k = v` will have `(Some(" "), Some(" "))`, whereas `k=\tv` will
    /// have `(None, Some("\t"))`.
    #[must_use]
    pub fn separator_whitespace(&self) -> (Option<&BStr>, Option<&BStr>) {
        (self.whitespace.pre_sep.as_deref(), self.whitespace.post_sep.as_deref())
    }
}

// Internal methods that may require exact indices for faster operations.
impl<'a, 'event> SectionMut<'a, 'event> {
    pub(crate) fn new(section: &'a mut Section<'event>, newline: SmallVec<[u8; 2]>) -> Self {
        let whitespace = Whitespace::from_body(&section.body);
        Self {
            section,
            implicit_newline: true,
            whitespace,
            newline,
        }
    }

    pub(crate) fn get<'key>(
        &self,
        key: &Key<'key>,
        start: Index,
        end: Index,
    ) -> Result<Cow<'_, BStr>, lookup::existing::Error> {
        let mut expect_value = false;
        let mut concatenated_value = BString::default();

        for event in &self.section.0[start.0..end.0] {
            match event {
                Event::SectionKey(event_key) if event_key == key => expect_value = true,
                Event::Value(v) if expect_value => return Ok(normalize_bstr(v.as_ref())),
                Event::ValueNotDone(v) if expect_value => {
                    concatenated_value.push_str(v.as_ref());
                }
                Event::ValueDone(v) if expect_value => {
                    concatenated_value.push_str(v.as_ref());
                    return Ok(normalize_bstring(concatenated_value));
                }
                _ => (),
            }
        }

        Err(lookup::existing::Error::KeyMissing)
    }

    pub(crate) fn delete(&mut self, start: Index, end: Index) {
        self.section.body.0.drain(start.0..end.0);
    }

    pub(crate) fn set_internal(&mut self, index: Index, key: Key<'event>, value: &BStr) -> Size {
        let mut size = 0;

        let body = &mut self.section.body.0;
        body.insert(index.0, Event::Value(escape_value(value).into()));
        size += 1;

        let sep_events = self.whitespace.key_value_separators();
        size += sep_events.len();
        body.insert_many(index.0, sep_events.into_iter().rev());

        body.insert(index.0, Event::SectionKey(key));
        size += 1;

        Size(size)
    }

    /// Performs the removal, assuming the range is valid.
    fn remove_internal(&mut self, range: Range<usize>) -> Cow<'event, BStr> {
        self.section
            .body
            .0
            .drain(range)
            .fold(Cow::Owned(BString::default()), |mut acc, e| {
                if let Event::Value(v) | Event::ValueNotDone(v) | Event::ValueDone(v) = e {
                    acc.to_mut().extend(&**v);
                }
                acc
            })
    }
}

impl<'event> Deref for SectionMut<'_, 'event> {
    type Target = file::Section<'event>;

    fn deref(&self) -> &Self::Target {
        self.section
    }
}

impl<'event> file::section::Body<'event> {
    pub(crate) fn as_mut(&mut self) -> &mut parse::section::Events<'event> {
        &mut self.0
    }
}
