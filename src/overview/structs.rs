use std::{
    fmt::Display,
    ops::{Deref, Range},
};

use syn::{spanned::Spanned, ItemStruct, Visibility};

use crate::{
    collect_src_maps, ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View,
    ViewableDiff, ViewableDiffs, VisDiff,
};

use super::{
    fields::{Fields, FieldsDiff},
    generics::{Generics, GenericsDiff},
};

const NO_SRC_ERROR: &str = "No source text for struct, was parse logic changed?";

/// A collection of `struct` declarations.
///
/// The internal representation is sorted and deduped.
#[derive(Debug)]
pub struct Structs(pub Vec<Struct>);
impl Structs {
    /// Creates a complete set of `struct` declarations from a list of `Struct`s.
    pub fn from(mut structs: Vec<Struct>) -> Self {
        structs.sort_by(|s1, s2| s1.name().cmp(&s2.name()));
        structs.dedup_by(|s1, s2| s1.name() == s2.name());
        Structs(structs)
    }
}
impl Diff for Structs {
    type Diff = StructsDiff;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        debug_assert!(self.0.is_sorted_by(|s1, s2| s1.name() <= s2.name()));
        debug_assert!(other.0.is_sorted_by(|s1, s2| s1.name() <= s2.name()));

        let mut struct_diffs = Vec::new();

        // file1
        for struct_ in &self.0 {
            match other.0.binary_search_by(|s| s.name().cmp(struct_.name())) {
                Ok(s) => {
                    if let Some(diff) = struct_.diff_with(&other.0[s]) {
                        struct_diffs.push(diff);
                    }
                }

                Err(_e) => {
                    // struct was deleted
                    let sdiff = StructDiff {
                        name: struct_.name().to_string(),
                        change: Change::Existence(ExistenceChange::Deleted),
                        old: Some(struct_.original.clone()),
                        old_src: Some(struct_.source.clone()),
                        ..Default::default()
                    };
                    struct_diffs.push(sdiff);
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for struct_ in &other.0 {
            if let Err(_e) = self.0.binary_search_by(|s| s.name().cmp(struct_.name())) {
                let sdiff = StructDiff {
                    name: struct_.name().to_string(),
                    change: Change::Existence(ExistenceChange::Added),
                    new: Some(struct_.original.clone()),
                    new_src: Some(struct_.source.clone()),
                    ..Default::default()
                };
                struct_diffs.push(sdiff);
            }
        }

        struct_diffs.sort_by(|d1, d2| d1.name.cmp(&d2.name));

        StructsDiff {
            structs: struct_diffs,
        }
    }
}
impl Deref for Structs {
    type Target = [Struct];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Struct {
    name: String,
    vis: Visibility,
    fields: Fields,
    generics: Generics,
    original: ItemStruct,
    source: SourceFile,
}
impl Struct {
    pub fn new(s: ItemStruct, source: SourceFile) -> Self {
        let original = s.clone();
        let vis = s.vis;
        let name = s.ident.to_string();
        let fields = s.fields.into_iter().collect();
        let fields = Fields(fields);
        let generics = Generics::from(s.generics.clone());

        Self {
            name,
            vis,
            fields,
            generics,
            original,
            source,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl Diff for Struct {
    type Diff = Option<StructDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        let self_name = self.name();
        let other_name = other.name();

        if self == other {
            return None;
        }

        if self_name != other_name {
            // two different structs
            return None;
        }

        let vis_diff = self.vis.diff_with(&other.vis);
        let fields_diff = self.fields.diff_with(&other.fields);
        let generics_diff = self.generics.diff_with(&other.generics);

        if vis_diff.is_none() && fields_diff.is_none() && generics_diff.is_none() {
            return None;
        }

        // take all the diffs, if old/new exists, get byte range and store in vec
        let (old_src_map, new_src_map) = collect_src_maps!(vis_diff, fields_diff, generics_diff,);

        Some(StructDiff {
            name: self_name.to_string(),
            change: Change::Modified,
            old: Some(self.original.clone()),
            new: Some(other.original.clone()),
            vis_diff,
            fields_diff,
            generics_diff,
            old_src: Some(self.source.clone()),
            new_src: Some(other.source.clone()),
            old_src_map,
            new_src_map,
        })
    }
}
impl Display for Struct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vis = self.vis.span().source_text().unwrap();
        write!(f, "{vis} struct {}", self.name)
    }
}

/// A collection of diffs for `struct` declarations.
pub struct StructsDiff {
    structs: Vec<StructDiff>,
}
impl StructsDiff {
    pub(crate) fn is_empty(&self) -> bool {
        self.structs.is_empty()
    }
}
impl View for StructsDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        let ex_diffs = self
            .structs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Existence(_)));

        let mut viewables = ViewableDiffs::empty();
        for ex_diff in ex_diffs {
            viewables.appendln(ex_diff.as_viewable());
        }

        // add/delete diffs should be side-by-side
        viewables.collapse();

        let mod_diffs = self
            .structs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for mod_diff in mod_diffs {
            viewables.appendln(mod_diff.as_viewable());
        }

        viewables
    }
}

/// A diff between two struct declarations.
#[derive(Debug, Default)]
#[allow(unused)]
pub struct StructDiff {
    name: String,
    change: Change,
    // present if the struct was deleted or modified
    old: Option<ItemStruct>,
    // present if the struct was added or modified
    new: Option<ItemStruct>,
    vis_diff: Option<VisDiff>,
    fields_diff: Option<FieldsDiff>,
    generics_diff: Option<GenericsDiff>,
    old_src: Option<SourceFile>,
    new_src: Option<SourceFile>,
    old_src_map: Vec<Range<usize>>,
    new_src_map: Vec<Range<usize>>,
}
impl View for StructDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        if let Change::Existence(ex) = self.change {
            let _struct = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new structs were both Some"),
                (None, None) => panic!("old and new structs were both None"),
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
        let decl_end = match &old.fields {
            syn::Fields::Named(fields_named) => fields_named.brace_token.span.span().byte_range().start + 1,
            syn::Fields::Unnamed(fields_unnamed) => fields_unnamed.paren_token.span.span().byte_range().start + 1,
            syn::Fields::Unit => old.span().byte_range().end,
        };

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
        
        if let Some(field_diffs) = &self.fields_diff {
            let mut i = decl_end;

            while i < old_range.end {
                let maybe_item_diff = field_diffs.diffs().iter().find(|d| d.old().as_ref().map(|old_variant| old_variant.span().byte_range().contains(&i)).unwrap_or(false));
                match maybe_item_diff {
                    Some(id) => {
                        let viewable = id.as_viewable();
                        for diff in viewable.vds {
                            if let Some(old) = diff.old {
                                old_diff.extend(old);
                            }
                        }

                        i = id.old().as_ref().unwrap().span().byte_range().end;
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

        match &old.fields {
            syn::Fields::Named(_) => old_diff.push((None, Code("}\n".to_string()))),
            syn::Fields::Unnamed(_) => old_diff.push((None, Code(")\n".to_string()))),
            _ => {}
        }

        let new = self.new.as_ref().unwrap();
        let new_src = &self.new_src.as_ref().unwrap().0.as_bytes();

        let new_range = new.span().byte_range();
        let decl_start = new_range.start;
        let decl_end = match &new.fields {
            syn::Fields::Named(fields_named) => fields_named.brace_token.span.span().byte_range().start + 1,
            syn::Fields::Unnamed(fields_unnamed) => fields_unnamed.paren_token.span.span().byte_range().start + 1,
            syn::Fields::Unit => new.span().byte_range().end,
        };

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

        if let Some(fds) = &self.fields_diff {
            let mut i = decl_end;
            while i < new_range.end {
                let maybe_item_diff = fds.diffs().iter().find(|d| d.new().as_ref().map(|new_func| new_func.span().byte_range().contains(&i)).unwrap_or(false));
                match maybe_item_diff {
                    Some(id) => {
                        let viewable = id.as_viewable();
                        for diff in viewable.vds {
                            if let Some(new) = diff.new {
                                new_diff.extend(new);
                            }
                        }

                        i = id.new().as_ref().unwrap().span().byte_range().end;
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

        match &new.fields {
            syn::Fields::Named(_) => new_diff.push((None, Code("}\n".to_string()))),
            syn::Fields::Unnamed(_) => new_diff.push((None, Code(")\n".to_string()))),
            _ => {}
        }

        ViewableDiffs::new(vec![ViewableDiff {
            old: Some(old_diff),
            new: Some(new_diff),
        }])
    }
}
