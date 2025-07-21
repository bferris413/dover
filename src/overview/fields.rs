use std::ops::Range;

use crate::{ByteRange, Change, Code, Diff, ExistenceChange, View, ViewableDiff, ViewableDiffs};

use syn::{FieldMutability, Type, spanned::Spanned};

const NO_SRC_ERROR: &str = "No source text for field, was parse logic changed?";

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Fields(pub syn::Fields);
impl Diff for Fields {
    type Diff = Option<FieldsDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        use syn::Fields::*;

        if self == other {
            return None;
        }

        match (&self.0, &other.0) {
            (Named(old_named), Named(new_named)) => {
                // field structs, order doesn't matter
                let mut old_named = old_named.named.iter().collect::<Vec<_>>();
                let mut new_named = new_named.named.iter().collect::<Vec<_>>();

                old_named.sort_by(|f1, f2| f1.ident.cmp(&f2.ident));
                new_named.sort_by(|f1, f2| f1.ident.cmp(&f2.ident));

                let mut field_diffs = Vec::new();

                for field in &old_named {
                    match new_named.binary_search_by(|new_f| {
                        new_f
                            .ident
                            .as_ref()
                            .unwrap()
                            .cmp(field.ident.as_ref().unwrap())
                    }) {
                        Ok(new_field_i) => {
                            if let Some(diff) = field.diff_with(&new_named[new_field_i]) {
                                field_diffs.push(diff);
                            }
                        }

                        Err(_e) => {
                            // field was deleted
                            let fdiff = FieldDiff {
                                old: Some((*field).clone()),
                                new: None,
                                change: Change::Existence(ExistenceChange::Deleted),
                            };
                            field_diffs.push(fdiff);
                        }
                    }
                }

                // Everything here is either new or already accounted for
                for field_ in &new_named {
                    if let Err(_e) = old_named.binary_search_by(|f| {
                        f.ident
                            .as_ref()
                            .unwrap()
                            .cmp(&field_.ident.as_ref().unwrap())
                    }) {
                        let fdiff = FieldDiff {
                            old: None,
                            new: Some((*field_).clone()),
                            change: Change::Existence(ExistenceChange::Added),
                        };
                        field_diffs.push(fdiff);
                    }
                }

                Some(FieldsDiff { diffs: field_diffs })
            }
            (Named(old_named), Unnamed(new_unnamed)) => {
                let mut diffs = Vec::new();
                // we could diff the types and assume the names don't matter, but for now
                // we'll just consider old: deleted and new: added. Maybe it could be user
                // configurable when we add .toml configs

                for old_field in old_named.named.iter() {
                    let fdiff = FieldDiff {
                        old: Some(old_field.clone()),
                        new: None,
                        change: Change::Existence(ExistenceChange::Deleted),
                    };
                    diffs.push(fdiff);
                }

                for new_field in new_unnamed.unnamed.iter() {
                    let fdiff = FieldDiff {
                        new: Some(new_field.clone()),
                        old: None,
                        change: Change::Existence(ExistenceChange::Added),
                    };
                    diffs.push(fdiff);
                }

                Some(FieldsDiff { diffs })
            }
            (Unnamed(old_unnamed), Named(new_named)) => {
                let mut diffs = Vec::new();
                // we could diff the types and assume the names don't matter, but for now
                // we'll just consider old: deleted and new: added. Maybe it could be user
                // configurable when we add .toml configs

                for old_field in old_unnamed.unnamed.iter() {
                    let fdiff = FieldDiff {
                        old: Some(old_field.clone()),
                        new: None,
                        change: Change::Existence(ExistenceChange::Deleted),
                    };
                    diffs.push(fdiff);
                }

                for new_field in new_named.named.iter() {
                    let fdiff = FieldDiff {
                        new: Some(new_field.clone()),
                        old: None,
                        change: Change::Existence(ExistenceChange::Added),
                    };
                    diffs.push(fdiff);
                }

                Some(FieldsDiff { diffs })
            }
            (Named(old_named_fields), Unit) => {
                // all fields are old
                let diffs = old_named_fields
                    .named
                    .iter()
                    .map(|field| FieldDiff {
                        new: None,
                        old: Some(field.clone()),
                        change: Change::Existence(ExistenceChange::Deleted),
                    })
                    .collect::<Vec<_>>();

                let fields_diff = FieldsDiff { diffs };
                Some(fields_diff)
            }

            (Unnamed(old_unnamed), Unnamed(new_unnamed)) => {
                // tuple structs, order matters

                let old_unnamed = old_unnamed.unnamed.iter().collect::<Vec<_>>();
                let new_unnamed = new_unnamed.unnamed.iter().collect::<Vec<_>>();

                let mut i = 0;
                let mut diffs = Vec::with_capacity(usize::max(self.0.len(), other.0.len()));
                loop {
                    match (old_unnamed.get(i), new_unnamed.get(i)) {
                        (Some(f1), Some(f2)) => {
                            if let Some(diff) = f1.diff_with(f2) {
                                diffs.push(diff);
                            }
                        }
                        (Some(f1), None) => {
                            diffs.push(FieldDiff {
                                old: Some((*f1).clone()),
                                new: None,
                                change: Change::Existence(ExistenceChange::Deleted),
                            });
                        }
                        (None, Some(f2)) => {
                            diffs.push(FieldDiff {
                                old: None,
                                new: Some((*f2).clone()),
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
            (Unnamed(old_unnamed_fields), Unit) => {
                // all fields are old
                let diffs = old_unnamed_fields
                    .unnamed
                    .iter()
                    .map(|field| FieldDiff {
                        new: None,
                        old: Some(field.clone()),
                        change: Change::Existence(ExistenceChange::Deleted),
                    })
                    .collect::<Vec<_>>();

                let fields_diff = FieldsDiff { diffs };
                Some(fields_diff)
            }
            (Unit, Named(new_named_fields)) => {
                // all fields are new
                let diffs = new_named_fields
                    .named
                    .iter()
                    .map(|field| FieldDiff {
                        old: None,
                        new: Some(field.clone()),
                        change: Change::Existence(ExistenceChange::Added),
                    })
                    .collect::<Vec<_>>();

                let fields_diff = FieldsDiff { diffs };
                Some(fields_diff)
            }
            (Unit, Unnamed(new_unnamed_fields)) => {
                // all fields are new
                let diffs = new_unnamed_fields
                    .unnamed
                    .iter()
                    .map(|field| FieldDiff {
                        old: None,
                        new: Some(field.clone()),
                        change: Change::Existence(ExistenceChange::Added),
                    })
                    .collect::<Vec<_>>();

                let fields_diff = FieldsDiff { diffs };
                Some(fields_diff)
            }
            (Unit, Unit) => unreachable!(),
        }
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
