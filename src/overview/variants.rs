use std::ops::Range;

use syn::{Variant as SynVariant, spanned::Spanned};

use crate::{
    ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View, ViewableDiff, ViewableDiffs,
    collect_src_maps, overview::fields::FieldDiff,
};

use super::fields::{Fields, FieldsDiff};

const NO_SRC_ERROR: &str = "No source text for variant, was parse logic changed?";

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Variants(Vec<Variant>);
impl Variants {
    pub(crate) fn new(mut syn_vars: Vec<SynVariant>, source: SourceFile) -> Self {
        syn_vars.sort_by(|v1, v2| v1.ident.cmp(&v2.ident));
        let variants = syn_vars
            .into_iter()
            .map(|v| Variant::new(v, source.clone()))
            .collect();
        Variants(variants)
    }
}

impl Diff for Variants {
    type Diff = Option<VariantDiffs>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        debug_assert!(
            self.0
                .is_sorted_by(|v1, v2| v1.original.ident <= v2.original.ident)
        );
        debug_assert!(
            other
                .0
                .is_sorted_by(|v1, v2| v1.original.ident <= v2.original.ident)
        );

        let mut variant_diffs = Vec::new();

        // file1
        for variant in &self.0 {
            match other
                .0
                .binary_search_by(|s| s.original.ident.cmp(&variant.original.ident))
            {
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
            if let Err(_e) = self
                .0
                .binary_search_by(|s| s.original.ident.cmp(&variant.original.ident))
            {
                let sdiff = VariantDiff {
                    change: Change::Existence(ExistenceChange::Added),
                    new: Some(variant.clone()),
                    ..Default::default()
                };
                variant_diffs.push(sdiff);
            }
        }

        if variant_diffs.is_empty() {
            None
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
            original: v,
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
    pub fn len(&self) -> usize {
        self.diffs.len()
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

        let self_fields = Fields(self.original.fields.clone());
        let other_fields = Fields(other.original.fields.clone());
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
                old_src_map,
                new_src_map,
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

    #[allow(clippy::wrong_self_convention)]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(&self) -> Option<&Variant> {
        self.new.as_ref()
    }
}
impl View for VariantDiff {
    fn as_viewable(&self) -> crate::ViewableDiffs {
        if let Change::Existence(ex) = self.change {
            return collect_existence_diff_changes(&self.old, &self.new, ex);
        }

        let old_diff = collect_variant_diff_changes(
            self.old.as_ref().unwrap(),
            self.old_src.as_ref().unwrap().0.as_bytes(),
            &self.old_src_map,
            &self.fields_diff,
            ExistenceChange::Deleted,
        );

        let new_diff = collect_variant_diff_changes(
            self.new.as_ref().unwrap(),
            self.new_src.as_ref().unwrap().0.as_bytes(),
            &self.new_src_map,
            &self.fields_diff,
            ExistenceChange::Added,
        );

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

fn collect_existence_diff_changes(
    old: &Option<Variant>,
    new: &Option<Variant>,
    ex: ExistenceChange,
) -> ViewableDiffs {
    let variant_ = match (&old, &new) {
        (Some(_), Some(_)) => panic!("old and new variants were both Some"),
        (None, None) => panic!("old and new variants were both None"),
        (Some(variant_), None) | (None, Some(variant_)) => variant_,
    };

    let source = variant_.original.span().source_text().expect(NO_SRC_ERROR);
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

fn collect_variant_diff_changes(
    variant: &Variant,
    source_code: &[u8],
    source_map: &[Range<usize>],
    field_diffs: &Option<FieldsDiff>,
    ex: ExistenceChange,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let source_range = variant.original.span().byte_range();
    let decl_start = source_range.start;
    let sig_end = end_index_of_signature(variant);
    let fields_end = end_index_of_fields(variant);

    let mut diff_changes =
        crate::collect_diff_changes(source_code, source_map, decl_start, sig_end, ex);

    if let Some(field_diffs) = field_diffs {
        let (get_orig_field, get_sub_diff): (
            Box<dyn Fn(&FieldDiff) -> Option<&syn::Field>>,
            Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
        );
        match ex {
            ExistenceChange::Added => {
                get_orig_field = Box::new(|fd: &FieldDiff| fd.new());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.new);
            }
            ExistenceChange::Deleted => {
                get_orig_field = Box::new(|fd: &FieldDiff| fd.old());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.old);
            }
        };
        let diffs_as_changes = collect_field_diffs(
            source_code,
            variant,
            sig_end,
            field_diffs,
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
fn collect_field_diffs(
    // The full source code for the file we're parsing
    source_code: &[u8],
    // The byte range in the source code of the variant we're parsing
    variant: &Variant,
    // The index at which the signature ends
    sig_end: usize,
    // The field diffs for the file we're parsing
    fds: &FieldsDiff,
    // How to get the original syn::Field from a field diff (old or new method)
    get_original_field: Box<dyn Fn(&FieldDiff) -> Option<&syn::Field>>,
    // How to get sub diffs from a given viewable diff (old or new field)
    get_sub_diffs: Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let mut diffs = Vec::new();
    let mut i = sig_end;
    let variant_range = variant.original.span().byte_range();

    if variant.original.fields.len() > fds.len() {
        let elided_whitespace =
            crate::collect_elided_whitespace(sig_end, source_code, variant_range.end);
        diffs.push((None, Code(elided_whitespace)));
    }

    while i < variant_range.end {
        let maybe_item_diff = fds.diffs().iter().find(|d| {
            get_original_field(d)
                .as_ref()
                .map(|field| field.span().byte_range().contains(&i))
                .unwrap_or(false)
        });
        match maybe_item_diff {
            Some(id) => {
                // Going to walk backwards and get all preceding whitespace until a newline or a character
                let item_diff_range = get_original_field(id).as_ref().unwrap().span().byte_range();
                let item_diff_start = item_diff_range.start;
                let item_diff_end = item_diff_range.end;

                let whitespace = crate::collect_preceding_whitespace(source_code, item_diff_start);
                diffs.push((None, Code(whitespace)));

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

    // It's possible we don't have any diffs and yet the variant has fields.
    // In this case, we should add some visual cue to indicate we elided irrelevant fields.
    if diffs.is_empty() && variant.original.fields.is_empty() {
        let elided_whitespace =
            crate::collect_elided_whitespace(sig_end, source_code, variant_range.end);
        diffs.push((None, Code(elided_whitespace)));
    }

    diffs
}

fn end_index_of_signature(variant: &Variant) -> usize {
    match &variant.original.fields {
        syn::Fields::Named(fields_named) => {
            fields_named.brace_token.span.span().byte_range().start + 1
        }
        syn::Fields::Unnamed(fields_unnamed) => {
            fields_unnamed.paren_token.span.span().byte_range().start + 1
        }
        syn::Fields::Unit => variant.original.span().byte_range().end,
    }
}

fn end_index_of_fields(variant: &Variant) -> usize {
    match &variant.original.fields {
        syn::Fields::Named(fields_named) => fields_named.named.span().byte_range().end,
        syn::Fields::Unnamed(fields_unnamed) => fields_unnamed.unnamed.span().byte_range().end,
        syn::Fields::Unit => variant.original.span().byte_range().end,
    }
}
