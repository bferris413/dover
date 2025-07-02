use std::{
    fmt::Display,
    ops::{Deref, Range},
};

use syn::{ItemStruct, Visibility, spanned::Spanned};

use crate::{
    ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View, ViewableDiff, ViewableDiffs,
    VisDiff, collect_src_maps,
};

use super::{
    fields::{Fields, FieldsDiff},
    generics::{Generics, GenericsDiff},
};

const NO_SRC_ERROR: &str = "No source text for struct, was parse logic changed?";
const ASCII_LINE_FEED: u8 = 10;

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
            viewables.append(ex_diff.as_viewable());
        }

        // add/delete diffs should be side-by-side
        viewables.collapse();

        let mod_diffs = self
            .structs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for mod_diff in mod_diffs {
            viewables.append(mod_diff.as_viewable());
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
            return collect_existence_diff_changes(&self.old, &self.new, ex);
        }

        let old_diff = collect_struct_diff_changes(
            self.old.as_ref().unwrap(),
            &self.old_src.as_ref().unwrap().0.as_bytes(),
            &self.old_src_map,
            &self.fields_diff,
        );

        let new_diff = collect_struct_diff_changes(
            self.new.as_ref().unwrap(),
            &self.new_src.as_ref().unwrap().0.as_bytes(),
            &self.new_src_map,
            &self.fields_diff,
        );

        ViewableDiffs::new(vec![ViewableDiff {
            old: Some(old_diff),
            new: Some(new_diff),
        }])
    }
}

fn collect_existence_diff_changes(
    old: &Option<ItemStruct>,
    new: &Option<ItemStruct>,
    ex: ExistenceChange,
) -> ViewableDiffs {
    let struct_ = match (&old, &new) {
        (Some(_), Some(_)) => panic!("old and new structs were both Some"),
        (None, None) => panic!("old and new structs were both None"),
        (Some(_struct), None) | (None, Some(_struct)) => _struct,
    };

    let source = struct_.span().source_text().expect(NO_SRC_ERROR);
    let diff_changes = vec![(Some(ex), Code(format!("{source}\n")))];

    match ex {
        ExistenceChange::Deleted => ViewableDiffs::new(vec![ViewableDiff {
            old: Some(diff_changes),
            new: None,
        }]),
        ExistenceChange::Added => ViewableDiffs::new(vec![ViewableDiff {
            old: None,
            new: Some(diff_changes),
        }]),
    }
}

fn collect_struct_diff_changes(
    struct_: &ItemStruct,
    source_code: &[u8],
    source_map: &[Range<usize>],
    field_diffs: &Option<FieldsDiff>,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let source_range = struct_.span().byte_range();
    let decl_start = source_range.start;
    let sig_end = end_index_of_signature(struct_);
    let fields_end = end_index_of_fields(struct_);

    let mut i = decl_start;
    let mut src_i = 0;
    let mut diff_changes = Vec::new();

    while i < sig_end {
        let maybe_diff_index = source_map[src_i..].iter().position(|r| r.contains(&i));
        match maybe_diff_index {
            Some(diff_index) => {
                let diff_range = &source_map[src_i..][diff_index];

                // doesn't make sense that we wouldn't be aligned with the start of a range
                assert_eq!(i, diff_range.start);
                let substring = source_code[i..diff_range.end].to_vec();
                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                diff_changes.push((Some(ExistenceChange::Deleted), code));

                src_i = diff_index + 1;
                i = diff_range.end;
            }
            None => {
                let start = i;
                while i < sig_end {
                    let maybe_diff_index = source_map[src_i..].iter().position(|r| r.contains(&i));
                    if maybe_diff_index.is_some() {
                        break;
                    } else {
                        i += 1
                    }
                }
                // We're either off the end or we've found a new diff. Either way,
                // start..i contains our next range
                let substring = source_code[start..i].to_vec();
                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                diff_changes.push((None, code));
            }
        }
    }

    if let Some(field_diffs) = field_diffs {
        let diffs_as_changes =
            collect_field_diffs(source_code, &source_range, sig_end, field_diffs);
        diff_changes.extend(diffs_as_changes);
    }

    // collect remaining whitespace and closing ')' or '}'
    let code = String::from_utf8(source_code[fields_end..source_range.end].to_vec())
        .expect("Off a code boundary");
    diff_changes.push((None, Code(code)));

    diff_changes
}

/// Converts field diffs into spans of code indicating the changes, if any.
fn collect_field_diffs(
    // The full source code for the file we're parsing
    source_code: &[u8],
    // The byte range in the source code of the struct we're parsing
    new_struct_range: &Range<usize>,
    // The index at which the signature ends
    sig_end: usize,
    // The field diffs for the file we're parsing
    fds: &FieldsDiff,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let mut diffs = Vec::new();
    let mut i = sig_end;

    while i < new_struct_range.end {
        let maybe_item_diff = fds.diffs().iter().find(|d| {
            d.new()
                .as_ref()
                .map(|new_func| new_func.span().byte_range().contains(&i))
                .unwrap_or(false)
        });
        match maybe_item_diff {
            Some(id) => {
                // Going to walk backwards and get all preceding whitespace until a newline or a character
                let item_diff_start = id.new().as_ref().unwrap().span().byte_range().start;
                let mut item_diff_whitespace_start = item_diff_start as isize - 1;

                while item_diff_whitespace_start > 0 {
                    if source_code[item_diff_whitespace_start as usize].is_ascii_whitespace() {
                        if source_code[item_diff_whitespace_start as usize] == ASCII_LINE_FEED {
                            break;
                        } else {
                            item_diff_whitespace_start -= 1;
                        }
                    } else {
                        break;
                    }
                }

                if !source_code[item_diff_whitespace_start as usize].is_ascii_whitespace() {
                    // we hit a non-whitespace character which shouldn't be included in our output
                    item_diff_whitespace_start += 1;
                }

                let substring =
                    source_code[item_diff_whitespace_start as usize..item_diff_start].to_vec();
                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                diffs.push((None, code));

                // then get the actual diff
                let viewable = id.as_viewable();
                for diff in viewable.vds {
                    if let Some(new) = diff.new {
                        diffs.extend(new);
                    }
                }

                i = id.new().as_ref().unwrap().span().byte_range().end;
            }
            None => {
                i += 1;
            }
        }
    }

    diffs
}

fn end_index_of_signature(struct_: &ItemStruct) -> usize {
    match &struct_.fields {
        syn::Fields::Named(fields_named) => {
            fields_named.brace_token.span.span().byte_range().start + 1
        }
        syn::Fields::Unnamed(fields_unnamed) => {
            fields_unnamed.paren_token.span.span().byte_range().start + 1
        }
        syn::Fields::Unit => struct_.span().byte_range().end,
    }
}

fn end_index_of_fields(struct_: &ItemStruct) -> usize {
    match &struct_.fields {
        syn::Fields::Named(fields_named) => fields_named.named.span().byte_range().end,
        syn::Fields::Unnamed(fields_unnamed) => fields_unnamed.unnamed.span().byte_range().end,
        syn::Fields::Unit => struct_.span().byte_range().end,
    }
}
