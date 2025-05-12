use std::{
    fmt::Display,
    ops::{Deref, Range},
};

use syn::{spanned::Spanned, ItemEnum, Visibility};

use crate::{
    collect_src_maps, ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View,
    ViewableDiff, ViewableDiffs, VisDiff,
};

use super::{
    generics::{Generics, GenericsDiff},
    variants::{VariantDiffs, Variants},
};
const NO_SRC_ERROR: &str = "No source text for enum, was parse logic changed?";

/// A collection of `enum` declarations.
///
/// The internal representation is sorted and deduped.
#[derive(Debug)]
pub struct Enums(pub Vec<Enum>);
impl Enums {
    /// Creates a complete set of `enum` declarations from a list of `Enum`s.
    pub fn from(mut enums: Vec<Enum>) -> Self {
        enums.sort_by(|e1, e2| e1.name().cmp(&e2.name()));
        enums.dedup_by(|e1, e2| e1.name() == e2.name());
        Enums(enums)
    }
}
impl Diff for Enums {
    type Diff = EnumsDiff;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        debug_assert!(self.0.is_sorted_by(|e1, e2| e1.name() <= e2.name()));
        debug_assert!(other.0.is_sorted_by(|e1, e2| e1.name() <= e2.name()));

        let mut enum_diffs = Vec::new();

        // file1
        for enum_ in &self.0 {
            match other.0.binary_search_by(|e| e.name().cmp(enum_.name())) {
                Ok(e) => {
                    if let Some(diff) = enum_.diff_with(&other.0[e]) {
                        enum_diffs.push(diff);
                    }
                }

                Err(_e) => {
                    // enum was deleted
                    let ediff = EnumDiff {
                        name: enum_.name().to_string(),
                        change: Change::Existence(ExistenceChange::Deleted),
                        old: Some(enum_.original.clone()),
                        old_src: Some(enum_.source.clone()),
                        ..Default::default()
                    };
                    enum_diffs.push(ediff);
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for enum_ in &other.0 {
            if let Err(_e) = self
                .0
                .binary_search_by(|r#enum| r#enum.name().cmp(enum_.name()))
            {
                let sdiff = EnumDiff {
                    name: enum_.name().to_string(),
                    change: Change::Existence(ExistenceChange::Added),
                    new: Some(enum_.original.clone()),
                    new_src: Some(enum_.source.clone()),
                    ..Default::default()
                };
                enum_diffs.push(sdiff);
            }
        }

        enum_diffs.sort_by(|d1, d2| d1.name.cmp(&d2.name));

        EnumsDiff { enums: enum_diffs }
    }
}
impl Deref for Enums {
    type Target = [Enum];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Enum {
    name: String,
    vis: Visibility,
    variants: Variants,
    generics: Generics,
    original: ItemEnum,
    source: SourceFile,
}
impl Enum {
    pub fn new(e: ItemEnum, source: SourceFile) -> Self {
        let original = e.clone();
        let vis = e.vis.into();
        let name = e.ident.to_string();
        let variants: Vec<_> = e.variants.into_iter().collect();
        let variants = Variants::new(variants, source.clone());
        let generics = Generics::from(e.generics.clone());

        Self {
            name,
            vis,
            variants,
            generics,
            original,
            source,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl Diff for Enum {
    type Diff = Option<EnumDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        let self_name = self.name();
        let other_name = other.name();

        if self == other {
            return None;
        }

        if self_name != other_name {
            return None;
        }

        let vis_diff = self.vis.diff_with(&other.vis);
        let variants_diff = self.variants.diff_with(&other.variants);
        let generics_diff = self.generics.diff_with(&other.generics);

        if vis_diff.is_none() && variants_diff.is_none() && generics_diff.is_none() {
            None
        } else {
            let (old_src_map, new_src_map) =
                collect_src_maps!(vis_diff, variants_diff, generics_diff,);

            Some(EnumDiff {
                name: self_name.to_string(),
                change: Change::Modified,
                old: Some(self.original.clone()),
                new: Some(other.original.clone()),
                old_src: Some(self.source.clone()),
                new_src: Some(other.source.clone()),
                old_src_map,
                new_src_map,
                vis_diff,
                variants_diff,
                generics_diff,
            })
        }
    }
}
impl Display for Enum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vis = self.vis.span().source_text().unwrap();
        write!(f, "{vis} struct {}", self.name)
    }
}

/// A collection of diffs for `struct` declarations.
pub struct EnumsDiff {
    enums: Vec<EnumDiff>,
}
impl EnumsDiff {
    pub(crate) fn is_empty(&self) -> bool {
        self.enums.is_empty()
    }
}
impl View for EnumsDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        let ex_diffs = self
            .enums
            .iter()
            .filter(|diff| matches!(diff.change, Change::Existence(_)));

        let mut viewables = ViewableDiffs::empty();
        for ex_diff in ex_diffs {
            viewables.append(ex_diff.as_viewable());
        }

        // add/delete diffs should be side-by-side
        viewables.collapse();

        let mod_diffs = self
            .enums
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for mod_diff in mod_diffs {
            viewables.append(mod_diff.as_viewable());
        }

        viewables
    }
}

/// A diff between two enum declarations.
#[derive(Debug, Default)]
#[allow(unused)]
pub struct EnumDiff {
    name: String,
    change: Change,
    // present if the enum was deleted or modified
    old: Option<ItemEnum>,
    old_src: Option<SourceFile>,
    // present if the enum was added or modified
    new: Option<ItemEnum>,
    new_src: Option<SourceFile>,
    old_src_map: Vec<Range<usize>>,
    new_src_map: Vec<Range<usize>>,
    vis_diff: Option<VisDiff>,
    variants_diff: Option<VariantDiffs>,
    generics_diff: Option<GenericsDiff>,
}
impl View for EnumDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        if let Change::Existence(ex) = self.change {
            let _struct = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new enums were both Some"),
                (None, None) => panic!("old and new enums were both None"),
                (Some(_struct), None) | (None, Some(_struct)) => _struct,
            };

            let source = _struct.span().source_text().expect(NO_SRC_ERROR);
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

        let old_range = old.span().byte_range();
        let decl_start = old_range.start;
        let decl_end = old.brace_token.span.span().byte_range().start + 1;

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
        
        if let Some(vdiffs) = &self.variants_diff {
            let mut i = decl_end;

            while i < old_range.end {
                let maybe_item_diff = vdiffs.diffs().iter().find(|d| d.old().as_ref().map(|old_variant| old_variant.original().span().byte_range().contains(&i)).unwrap_or(false));
                match maybe_item_diff {
                    Some(id) => {
                        let viewable = id.as_viewable();
                        for diff in viewable.vds {
                            if let Some(old) = diff.old {
                                old_diff.extend(old);
                            }
                        }

                        i = id.old().as_ref().unwrap().original().span().byte_range().end;
                    }
                    None => {
                        if old_src[i].is_ascii_whitespace() {
                            let start = i;
                            while i < old_range.end && old_src[i].is_ascii_whitespace() {
                                i += 1;
                            }

                            let substring = old_src[start..i].to_vec();
                            let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                            old_diff.push((None, code));
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        }

        old_diff.push((None, Code("}\n".to_string())));

        let new = self.new.as_ref().unwrap();
        let new_src = &self.new_src.as_ref().unwrap().0.as_bytes();

        let new_range = new.span().byte_range();
        let decl_start = new_range.start;
        let decl_end = new.brace_token.span.span().byte_range().start + 1; // we'll take the "{"

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

        if let Some(vds) = &self.variants_diff {
            let mut i = decl_end;
            while i < new_range.end {
                let maybe_item_diff = vds.diffs().iter().find(|d| d.new().as_ref().map(|new_func| new_func.original().span().byte_range().contains(&i)).unwrap_or(false));
                match maybe_item_diff {
                    Some(id) => {
                        let viewable = id.as_viewable();
                        for diff in viewable.vds {
                            if let Some(new) = diff.new {
                                new_diff.extend(new);
                            }
                        }

                        i = id.new().as_ref().unwrap().original().span().byte_range().end;
                    }
                    None => {
                        if new_src[i].is_ascii_whitespace() {
                            let start = i;
                            while i < new_range.end && new_src[i].is_ascii_whitespace() {
                                i += 1;
                            }

                            let substring = new_src[start..i].to_vec();
                            let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                            new_diff.push((None, code));
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        }

        new_diff.push((None, Code("}\n".to_string())));

        ViewableDiffs::new(vec![ViewableDiff {
            old: Some(old_diff),
            new: Some(new_diff),
        }])
    }
}
