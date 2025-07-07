use std::{
    fmt::Display,
    ops::{Deref, Range},
};

use syn::{ItemEnum, Visibility, spanned::Spanned};

use crate::{
    ASCII_LINE_FEED, ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View,
    ViewableDiff, ViewableDiffs, VisDiff, collect_src_maps,
    overview::variants::{Variant, VariantDiff},
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
            return collect_existence_diff_changes(&self.old, &self.new, ex);
        }

        let old_diff = collect_enum_diff_changes(
            self.old.as_ref().unwrap(),
            &self.old_src.as_ref().unwrap().0.as_bytes(),
            &self.old_src_map,
            &self.variants_diff,
            ExistenceChange::Deleted,
        );

        let new_diff = collect_enum_diff_changes(
            self.new.as_ref().unwrap(),
            &self.new_src.as_ref().unwrap().0.as_bytes(),
            &self.new_src_map,
            &self.variants_diff,
            ExistenceChange::Added,
        );

        ViewableDiffs::new(vec![ViewableDiff {
            old: Some(old_diff),
            new: Some(new_diff),
        }])
    }
}

fn collect_existence_diff_changes(
    old: &Option<ItemEnum>,
    new: &Option<ItemEnum>,
    ex: ExistenceChange,
) -> ViewableDiffs {
    let _enum = match (old, new) {
        (Some(_), Some(_)) => panic!("old and new enums were both Some"),
        (None, None) => panic!("old and new enums were both None"),
        (Some(_enum), None) | (None, Some(_enum)) => _enum,
    };

    let source = _enum.span().source_text().expect(NO_SRC_ERROR);
    let change = vec![(Some(ex), Code(format!("{source}\n")))];
    match ex {
        ExistenceChange::Deleted => {
            return ViewableDiffs::new(vec![ViewableDiff {
                old: Some(change),
                new: None,
            }]);
        }
        ExistenceChange::Added => {
            return ViewableDiffs::new(vec![ViewableDiff {
                old: None,
                new: Some(change),
            }]);
        }
    }
}

fn collect_enum_diff_changes(
    enum_: &ItemEnum,
    source_code: &[u8],
    source_map: &[Range<usize>],
    variant_diffs: &Option<VariantDiffs>,
    ex: ExistenceChange,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let source_range = enum_.span().byte_range();
    let decl_start = source_range.start;
    let sig_end = end_index_of_signature(enum_);
    let fields_end = end_index_of_variants(enum_, source_code);

    let mut diff_changes =
        crate::collect_diff_changes(source_code, source_map, decl_start, sig_end);

    if let Some(variant_diffs) = variant_diffs {
        let (get_orig_field, get_sub_diff): (
            Box<dyn Fn(&VariantDiff) -> Option<&Variant>>,
            Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
        );
        match ex {
            ExistenceChange::Added => {
                get_orig_field = Box::new(|vd: &VariantDiff| vd.new());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.new);
            }
            ExistenceChange::Deleted => {
                get_orig_field = Box::new(|vd: &VariantDiff| vd.old());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.old);
            }
        };
        let diffs_as_changes = collect_variant_diffs(
            source_code,
            &source_range,
            sig_end,
            variant_diffs,
            get_orig_field,
            get_sub_diff,
        );
        diff_changes.extend(diffs_as_changes);
    }

    // collect remaining whitespace and closing ')' or '}'
    let code = String::from_utf8(source_code[fields_end..source_range.end].to_vec())
        .expect("Off a code boundary");

    diff_changes.push((None, Code(code)));
    diff_changes
}

/// Converts field diffs into spans of code indicating the changes, if any.
fn collect_variant_diffs(
    // The full source code for the file we're parsing
    source_code: &[u8],
    // The byte range in the source code of the enum we're parsing
    enum_range: &Range<usize>,
    // The index at which the signature ends
    sig_end: usize,
    // The variant diffs for the file we're parsing
    vds: &VariantDiffs,
    // How to get the original Variant from a variant diff (old or new method)
    get_original_variant: Box<dyn Fn(&VariantDiff) -> Option<&Variant>>,
    // How to get sub diffs from a given viewable diff (old or new field)
    get_sub_diffs: Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let mut diffs = Vec::new();
    let mut i = sig_end;

    while i < enum_range.end {
        let maybe_item_diff = vds.diffs().iter().find(|d| {
            get_original_variant(d)
                .as_ref()
                .map(|variant| variant.original().span().byte_range().contains(&i))
                .unwrap_or(false)
        });
        match maybe_item_diff {
            Some(id) => {
                // Going to walk backwards and get all preceding whitespace until a newline or a character
                let item_diff_range = get_original_variant(id)
                    .as_ref()
                    .unwrap()
                    .original()
                    .span()
                    .byte_range();
                let item_diff_start = item_diff_range.start;
                let item_diff_end = item_diff_range.end;

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
                    if let Some(sub_diff) = get_sub_diffs(diff) {
                        diffs.extend(sub_diff);
                    }
                }

                i = item_diff_end;
            }
            None => {
                i += 1;
            }
        }
    }

    diffs
}

fn end_index_of_signature(enum_: &ItemEnum) -> usize {
    enum_.brace_token.span.span().byte_range().start + 1
}

fn end_index_of_variants(enum_: &ItemEnum, source_code: &[u8]) -> usize {
    let mut index_before_close_brace = enum_.span().byte_range().end - 2;

    while index_before_close_brace > 0
        && source_code[index_before_close_brace].is_ascii_whitespace()
    {
        index_before_close_brace -= 1;
    }

    index_before_close_brace += 1; // we want 1 index beyond the last char we ended at
    index_before_close_brace
}
