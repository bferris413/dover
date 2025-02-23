use syn::Variant;

use crate::{Change, Diff};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Variants(pub Vec<Variant>);
impl Diff for Variants {
    type Diff = Option<VariantsDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        todo!()
    }
}

#[derive(Debug)]
pub struct VariantsDiff {
    diffs: Vec<Option<VariantDiff>>,
}
impl VariantsDiff {
    pub fn diffs(&self) -> &[Option<VariantDiff>] {
        &self.diffs
    }
}

impl Diff for Variant {
    type Diff = Option<VariantDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        todo!()
    }
}

#[derive(Debug)]
pub struct VariantDiff {
    change: Change,
    old: Option<Variant>,
    new: Option<Variant>,
}
impl VariantDiff {
    pub fn change(&self) -> Change {
        self.change
    }
    pub fn old(&self) -> Option<&Variant> {
        self.old.as_ref()
    }
    pub fn new(&self) -> Option<&Variant> {
        self.new.as_ref()
    }
}
