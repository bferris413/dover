use std::ops::Range;

use syn::{spanned::Spanned, Variant as SynVariant};

use crate::{collect_src_maps, ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View, ViewableDiff, ViewableDiffs};

use super::fields::{Fields, FieldsDiff};

const NO_SRC_ERROR: &str = "No source text for variant, was parse logic changed?";

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Variants(Vec<Variant>);
impl Variants {
    pub(crate) fn new(mut syn_vars: Vec<SynVariant>, source: SourceFile) -> Self {
        syn_vars.sort_by(|v1, v2| v1.ident.cmp(&v2.ident));
        let variants = syn_vars.into_iter().map(|v| Variant::new(v, source.clone())).collect();
        Variants(variants)
    }
}

impl Diff for Variants {
    type Diff = Option<VariantDiffs>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        debug_assert!(self.0.is_sorted_by(|v1, v2| v1.original.ident <= v2.original.ident));
        debug_assert!(other.0.is_sorted_by(|v1, v2| v1.original.ident <= v2.original.ident));

        let mut variant_diffs = Vec::new();

        // file1
        for variant in &self.0 {
            match other.0.binary_search_by(|s| s.original.ident.cmp(&variant.original.ident)) {
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
                        ..Default::default()
                    };
                    variant_diffs.push(vdiff);
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for variant in &other.0 {
            if let Err(_e) = self.0.binary_search_by(|s| s.original.ident.cmp(&variant.original.ident)) {
                let sdiff = VariantDiff {
                    change: Change::Existence(ExistenceChange::Added),
                    new: Some(variant.clone()),
                    ..Default::default()
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

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Variant {
    original: SynVariant,
    source: SourceFile,
}
impl Variant {
    fn new(v: SynVariant, source: SourceFile) -> Self {
        Self {
            source,
            original: v
        }
    }
    pub fn original(&self) -> &SynVariant {
        &self.original
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
impl View for VariantDiffs {
    fn as_viewable(&self) -> ViewableDiffs {
        let ex_diffs = self
            .diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Existence(_)));

        let mut viewables = ViewableDiffs::empty();
        for ex_diff in ex_diffs {
            viewables.append(ex_diff.as_viewable());
        }

        // add/delete diffs should be side-by-side
        viewables.collapse();

        let mod_diffs = self
            .diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for mod_diff in mod_diffs {
            viewables.append(mod_diff.as_viewable());
        }

        viewables
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
        if self.original.ident != other.original.ident {
            return None;
        }

        let self_fields = Fields(self.original.fields.clone().into_iter().collect());
        let other_fields = Fields(other.original.fields.clone().into_iter().collect());
        let fields_diff = self_fields.diff_with(&other_fields);

        let change = Change::Modified;
        let old = self.clone();
        let new = other.clone();

        if fields_diff.is_none() {
            None
        } else {
            let (old_src_map, new_src_map) = collect_src_maps!(fields_diff);
            Some(VariantDiff {
                change,
                fields_diff,
                old: Some(old),
                new: Some(new),
                old_src: Some(self.source.clone()),
                new_src: Some(other.source.clone()),
                old_src_map: old_src_map,
                new_src_map: new_src_map
            })
        }
    }
}

#[derive(Debug, Default)]
pub struct VariantDiff {
    change: Change,
    fields_diff: Option<FieldsDiff>,
    old: Option<Variant>,
    old_src: Option<SourceFile>,
    new: Option<Variant>,
    new_src: Option<SourceFile>,
    old_src_map: Vec<Range<usize>>,
    new_src_map: Vec<Range<usize>>,
}
impl VariantDiff {
    // pub fn change(&self) -> Change {
    //     self.change
    // }
    pub fn old(&self) -> Option<&Variant> {
        self.old.as_ref()
    }
    pub fn new(&self) -> Option<&Variant> {
        self.new.as_ref()
    }
}
impl View for VariantDiff {
    fn as_viewable(&self) -> crate::ViewableDiffs {
        if let Change::Existence(ex) = self.change {
            let v = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new variants were both Some"),
                (None, None) => panic!("old and new variants were both None"),
                (Some(i), None) | (None, Some(i)) => i,
            };

            let source = v.original.span().source_text().expect(NO_SRC_ERROR);
            let change = vec![(Some(ex), Code(format!("{source}\n")))];
            match ex {
                ExistenceChange::Deleted => {
                    return ViewableDiffs::new(vec![ViewableDiff {
                        old: Some(change),
                        new: None,
                    }])
                }
                ExistenceChange::Added => {
                    return ViewableDiffs::new(vec![ViewableDiff {
                        old: None,
                        new: Some(change),
                    }])
                }
            };
        }


        let old = self.old.as_ref().unwrap();
        let old_src = &self.old_src.as_ref().unwrap().0.as_bytes();

        let old_range = old.original.span().byte_range();
        let decl_start = old_range.start;
        let decl_end = old_range.end;

        let mut i = decl_start;
        let mut src_i = 0;
        let mut old_diff = Vec::new();

        while i < decl_end {
            let maybe_diff_index = self.old_src_map[src_i..]
                .iter()
                .position(|r| r.contains(&i));
            match maybe_diff_index {
                Some(diff_index) => {
                    let diff_range = &self.old_src_map[src_i..][diff_index];

                    // doesn't make sense that we wouldn't be aligned with the start of a range
                    assert_eq!(i, diff_range.start);
                    let substring = old_src[i..diff_range.end].to_vec();
                    let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                    old_diff.push((Some(ExistenceChange::Deleted), code));

                    src_i = diff_index + 1;
                    i = diff_range.end;
                }
                None => {
                    let start = i;
                    while i < decl_end {
                        let maybe_diff_index = self.old_src_map[src_i..]
                            .iter()
                            .position(|r| r.contains(&i));
                        if maybe_diff_index.is_some() {
                            break;
                        } else {
                            i += 1
                        }
                    }
                    // We're either off the end or we've found a new diff. Either way,
                    // start..i contains our next range
                    let substring = old_src[start..i].to_vec();
                    let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                    old_diff.push((None, code));
                }
            }
        }

        let new = self.new.as_ref().unwrap();
        let new_src = &self.new_src.as_ref().unwrap().0.as_bytes();

        let new_range = new.original.span().byte_range();
        let decl_start = new_range.start;
        let decl_end = new_range.end;

        let mut i = decl_start;
        let mut src_i = 0;
        let mut new_diff = Vec::new();

        while i < decl_end {
            let maybe_diff_index = self.new_src_map[src_i..]
                .iter()
                .position(|r| r.contains(&i));
            match maybe_diff_index {
                Some(diff_index) => {
                    let diff_range = &self.new_src_map[src_i..][diff_index];

                    // doesn't make sense that we wouldn't be aligned with the start of a range
                    assert_eq!(i, diff_range.start);
                    let substring = new_src[i..diff_range.end].to_vec();
                    let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                    new_diff.push((Some(ExistenceChange::Added), code));

                    src_i = diff_index + 1;
                    i = diff_range.end;
                }
                None => {
                    let start = i;
                    while i < decl_end {
                        let maybe_diff_index = self.new_src_map[src_i..]
                            .iter()
                            .position(|r| r.contains(&i));
                        if maybe_diff_index.is_some() {
                            break;
                        } else {
                            i += 1
                        }
                    }
                    // We're either off the end or we've found a new diff. Either way,
                    // start..i contains our next range
                    let substring = new_src[start..i].to_vec();
                    let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                    new_diff.push((None, code));
                }
            }
        }

        ViewableDiffs::new(vec![ViewableDiff {
            old: Some(old_diff),
            new: Some(new_diff),
        }])
    }
}
impl ByteRange for VariantDiff {
    fn old_ranges(&self) -> Vec<std::ops::Range<usize>> {
        if let Some(fd) = &self.fields_diff {
            fd.old_ranges()
        } else if let Some(old) = &self.old {
            vec![old.original.span().byte_range()]
        } else {
            vec![]
        }
    }

    fn new_ranges(&self) -> Vec<std::ops::Range<usize>> {
        if let Some(fd) = &self.fields_diff {
            fd.new_ranges()
        } else if let Some(new) = &self.new {
            vec![new.original.span().byte_range()]
        } else {
            vec![]
        }
    }
}
