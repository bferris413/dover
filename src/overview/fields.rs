use crate::{Change, Diff, ExistenceChange};

use super::structs::{Vis, VisDiff};
use syn::{FieldMutability, Type};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Fields(pub Vec<Field>);
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
                    diffs.push(Some(FieldDiff::new(s1.clone(), ExistenceChange::Deleted)));
                }
                (None, Some(s2)) => {
                    diffs.push(Some(FieldDiff::new(s2.clone(), ExistenceChange::Added)));
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Field {
    name: Option<String>,
    vis: Vis,
    mutability: FieldMutability,
    ty: Type,
}
impl Diff for Field {
    type Diff = Option<FieldDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let name_diff = if self.name != other.name {
            Some(NameDiff {
                old: self.name.clone(),
                new: other.name.clone(),
            })
        } else {
            None
        };

        let vis_diff = if self.vis != other.vis {
            Some(VisDiff {
                old: self.vis.clone(),
                new: other.vis.clone(),
            })
        } else {
            None
        };

        let mut_diff = if self.mutability != other.mutability {
            Some(MutabilityDiff {
                old: self.mutability.clone(),
                new: other.mutability.clone(),
            })
        } else {
            None
        };

        let type_diff = if self.ty != other.ty {
            Some(TypeDiff {
                old: self.ty.clone(),
                new: other.ty.clone(),
            })
        } else {
            None
        };

        let change = Change::Modified;
        let diff = FieldDiff {
            name_diff,
            vis_diff,
            mut_diff,
            change,
            field: None,
            type_diff,
        };

        Some(diff)
    }
}
impl From<syn::Field> for Field {
    fn from(field: syn::Field) -> Self {
        let name = field.ident.map(|ident| ident.to_string());
        let vis = field.vis.into();
        let ty = field.ty;
        let mutability = field.mutability;

        Field {
            name,
            vis,
            ty,
            mutability,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct FieldDiff {
    name_diff: Option<NameDiff>,
    vis_diff: Option<VisDiff>,
    mut_diff: Option<MutabilityDiff>,
    change: Change,
    field: Option<Field>,
    type_diff: Option<TypeDiff>,
}
impl FieldDiff {
    fn new(field: Field, change: ExistenceChange) -> Self {
        Self {
            name_diff: None,
            vis_diff: None,
            mut_diff: None,
            change: Change::Existence(change),
            field: Some(field),
            type_diff: None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct TypeDiff {
    old: Type,
    new: Type,
}

#[derive(Debug, Eq, PartialEq)]
struct NameDiff {
    old: Option<String>,
    new: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct MutabilityDiff {
    old: FieldMutability,
    new: FieldMutability,
}
