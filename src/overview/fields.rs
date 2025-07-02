use std::ops::Range;

use crate::{ByteRange, Change, Code, Diff, ExistenceChange, View, ViewableDiff, ViewableDiffs};

use syn::{FieldMutability, Type, spanned::Spanned};

const NO_SRC_ERROR: &str = "No source text for field, was parse logic changed?";

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Fields(pub Vec<syn::Field>);
impl Diff for Fields {
    type Diff = Option<FieldsDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let mut i = 0;
        let mut diffs = Vec::with_capacity(usize::max(self.0.len(), other.0.len()));
        loop {
            match (self.0.get(i), other.0.get(i)) {
                (Some(f1), Some(f2)) => {
                    if let Some(diff) = f1.diff_with(f2) {
                        diffs.push(diff);
                    }
                }
                (Some(f1), None) => {
                    diffs.push(FieldDiff {
                        old: Some(f1.clone()),
                        new: None,
                        change: Change::Existence(ExistenceChange::Deleted),
                    });
                }
                (None, Some(f2)) => {
                    diffs.push(FieldDiff {
                        new: Some(f2.clone()),
                        old: None,
                        change: Change::Existence(ExistenceChange::Added),
                    });
                }
                (None, None) => break,
            }

            i += 1;
        }

        let fields_diff = FieldsDiff { diffs };
        Some(fields_diff)
    }
}

/// Represents the diff of a struct's fields.
///
/// This is a list of field diffs, where each field diff is either `None` (no change) or a `FieldDiff`.
#[derive(Debug, Eq, PartialEq)]
pub struct FieldsDiff {
    diffs: Vec<FieldDiff>,
}
impl ByteRange for FieldsDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = vec![];
        for diff in &self.diffs {
            let old_ranges = diff.old_ranges();
            ranges.extend(old_ranges);
        }

        ranges
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = vec![];
        for diff in &self.diffs {
            let new_ranges = diff.new_ranges();
            ranges.extend(new_ranges);
        }

        ranges
    }
}
#[allow(unused)]
impl FieldsDiff {
    pub fn diffs(&self) -> &[FieldDiff] {
        &self.diffs
    }
    pub fn len(&self) -> usize {
        self.diffs.len()
    }
}

impl Diff for syn::Field {
    type Diff = Option<FieldDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }
        let change = Change::Modified;
        let old = self.clone();
        let new = other.clone();

        Some(FieldDiff {
            change,
            old: Some(old),
            new: Some(new),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct FieldDiff {
    change: Change,
    old: Option<syn::Field>,
    new: Option<syn::Field>,
}
impl FieldDiff {
    pub fn change(&self) -> Change {
        self.change
    }
    pub fn old(&self) -> Option<&syn::Field> {
        self.old.as_ref()
    }
    pub fn new(&self) -> Option<&syn::Field> {
        self.new.as_ref()
    }
}
impl View for FieldDiff {
    fn as_viewable(&self) -> crate::ViewableDiffs {
        let old = self.old().map(|field| {
            let source = field.span().source_text().expect(NO_SRC_ERROR);
            vec![(Some(ExistenceChange::Deleted), Code(format!("{source}")))]
        });
        let new = self.new().map(|field| {
            let source = field.span().source_text().expect(NO_SRC_ERROR);
            vec![(Some(ExistenceChange::Added), Code(format!("{source}")))]
        });

        ViewableDiffs::new(vec![ViewableDiff { old, new }])
    }
}
impl ByteRange for FieldDiff {
    fn old_ranges(&self) -> Vec<std::ops::Range<usize>> {
        self.old
            .as_ref()
            .map(|field| vec![field.span().byte_range()])
            .unwrap_or_default()
    }

    fn new_ranges(&self) -> Vec<std::ops::Range<usize>> {
        self.new
            .as_ref()
            .map(|field| vec![field.span().byte_range()])
            .unwrap_or_default()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct TypeDiff {
    old: Type,
    new: Type,
}

#[derive(Debug, Eq, PartialEq)]
pub struct NameDiff {
    old: Option<String>,
    new: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MutabilityDiff {
    old: FieldMutability,
    new: FieldMutability,
}
