use crate::{Change, Diff, ExistenceChange};

use syn::{FieldMutability, Type};

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
                (Some(s1), Some(s2)) => {
                    diffs.push(s1.diff_with(s2));
                }
                (Some(s1), None) => {
                    diffs.push(Some(FieldDiff {
                        old: Some(s1.clone()),
                        new: None,
                        change: Change::Existence(ExistenceChange::Deleted),
                    }));
                }
                (None, Some(s2)) => {
                    diffs.push(Some(FieldDiff {
                        new: Some(s2.clone()),
                        old: None,
                        change: Change::Existence(ExistenceChange::Added),
                    }));
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
    diffs: Vec<Option<FieldDiff>>,
}
impl FieldsDiff {
    pub fn diffs(&self) -> &[Option<FieldDiff>] {
        &self.diffs
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
