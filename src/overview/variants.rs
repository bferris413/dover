use syn::{spanned::Spanned, Variant};

use crate::{ByteRange, Change, Diff, ExistenceChange};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Variants(Vec<Variant>);
impl From<Vec<Variant>> for Variants {
    fn from(mut variants: Vec<Variant>) -> Self {
        variants.sort_by(|v1, v2| v1.ident.cmp(&v2.ident));
        Variants(variants)
    }
}

impl Diff for Variants {
    type Diff = Option<VariantDiffs>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        debug_assert!(self.0.is_sorted_by(|v1, v2| v1.ident <= v2.ident));
        debug_assert!(other.0.is_sorted_by(|v1, v2| v1.ident <= v2.ident));

        let mut variant_diffs = Vec::new();

        // file1
        for variant in &self.0 {
            match other.0.binary_search_by(|s| s.ident.cmp(&variant.ident)) {
                Ok(s) => {
                    if let Some(diff) = variant.diff_with(&other.0[s]) {
                        variant_diffs.push(diff);
                    }
                }

                Err(_e) => {
                    // variant was deleted
                    let vdiff = VariantDiff {
                        change: Change::Existence(ExistenceChange::Deleted),
                        old: Some(variant.clone()),
                        new: None,
                    };
                    variant_diffs.push(vdiff);
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for variant in &other.0 {
            if let Err(_e) = self.0.binary_search_by(|s| s.ident.cmp(&variant.ident)) {
                let sdiff = VariantDiff {
                    change: Change::Existence(ExistenceChange::Added),
                    old: None,
                    new: Some(variant.clone()),
                };
                variant_diffs.push(sdiff);
            }
        }

        if variant_diffs.is_empty() {
            return None;
        } else {
            let variants_diff = VariantDiffs {
                diffs: variant_diffs,
            };
            Some(variants_diff)
        }
    }
}

#[derive(Debug)]
pub struct VariantDiffs {
    diffs: Vec<VariantDiff>,
}
impl VariantDiffs {
    pub fn diffs(&self) -> &[VariantDiff] {
        &self.diffs
    }
}
impl ByteRange for VariantDiffs {
    fn old_ranges(&self) -> Vec<std::ops::Range<usize>> {
        let mut ranges = vec![];
        for diff in self.diffs() {
            ranges.extend(diff.old_ranges());
        }

        ranges
    }

    fn new_ranges(&self) -> Vec<std::ops::Range<usize>> {
        let mut ranges = vec![];
        for diff in self.diffs() {
            ranges.extend(diff.new_ranges());
        }

        ranges
    }
}

impl Diff for Variant {
    type Diff = Option<VariantDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }
        let change = Change::Modified;
        let old = self.clone();
        let new = other.clone();

        Some(VariantDiff {
            change,
            old: Some(old),
            new: Some(new),
        })
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
impl ByteRange for VariantDiff {
    fn old_ranges(&self) -> Vec<std::ops::Range<usize>> {
        if let Some(old) = &self.old {
            vec![old.span().byte_range()]
        } else {
            vec![]
        }
    }

    fn new_ranges(&self) -> Vec<std::ops::Range<usize>> {
        if let Some(new) = &self.new {
            vec![new.span().byte_range()]
        } else {
            vec![]
        }
    }
}
