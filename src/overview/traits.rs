use std::{
    fmt::Display,
    ops::{Deref, Range},
};

use syn::{Item, ItemTrait, TraitItem, Visibility, spanned::Spanned};

use super::{
    functions::{Functions, FunctionsDiff},
    generics::{Generics, GenericsDiff},
};
use crate::{
    ASCII_LINE_FEED, ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View,
    ViewableDiff, ViewableDiffs, VisDiff, collect_src_maps, get_source,
    overview::functions::{Function, FunctionDiff},
};

const NO_SRC_ERROR: &str = "No source text for trait, was parse logic changed?";

#[derive(Debug)]
pub struct Traits(pub Vec<Trait>);
impl Traits {
    /// Creates a complete set of `struct` declarations from a list of `Trait`s.
    pub fn from(mut traits: Vec<Trait>) -> Self {
        traits.sort_by(|t1, t2| t1.name().cmp(&t2.name()));
        traits.dedup_by(|t1, t2| t1.name() == t2.name());
        Traits(traits)
    }
}
impl Diff for Traits {
    type Diff = TraitsDiff;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        debug_assert!(self.0.is_sorted_by(|t1, t2| t1.name() <= t2.name()));
        debug_assert!(other.0.is_sorted_by(|t1, t2| t1.name() <= t2.name()));

        let mut trait_diffs = Vec::new();

        // file1
        for trait_ in &self.0 {
            match other.0.binary_search_by(|t| t.name().cmp(trait_.name())) {
                Ok(t) => {
                    if let Some(diff) = trait_.diff_with(&other.0[t]) {
                        trait_diffs.push(diff);
                    }
                }

                Err(_e) => {
                    // trait was deleted
                    let sdiff = TraitDiff {
                        name: trait_.name().to_string(),
                        change: Change::Existence(ExistenceChange::Deleted),
                        old: Some(trait_.original.clone()),
                        old_src: Some(trait_.source.clone()),
                        ..Default::default()
                    };
                    trait_diffs.push(sdiff);
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for trait_ in &other.0 {
            if let Err(_e) = self.0.binary_search_by(|t| t.name().cmp(trait_.name())) {
                let tdiff = TraitDiff {
                    name: trait_.name().to_string(),
                    change: Change::Existence(ExistenceChange::Added),
                    new: Some(trait_.original.clone()),
                    new_src: Some(trait_.source.clone()),
                    ..Default::default()
                };
                trait_diffs.push(tdiff);
            }
        }

        trait_diffs.sort_by(|d1, d2| d1.name.cmp(&d2.name));

        TraitsDiff { diffs: trait_diffs }
    }
}
impl Deref for Traits {
    type Target = [Trait];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Trait {
    name: String,
    vis: Visibility,
    generics: Generics,
    items: Vec<TraitItem>,
    original: ItemTrait,
    source: SourceFile,
}
impl Trait {
    pub fn new(t: ItemTrait, source: SourceFile) -> Self {
        let original = t.clone();
        let name = t.ident.to_string();
        let vis = t.vis;
        let generics = Generics::from(t.generics);
        let items = t.items;
        Trait {
            name,
            vis,
            generics,
            items,
            original,
            source,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl Display for Trait {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let source = get_source(vec![Item::Trait(self.original.clone())]);
        write!(f, "{}", source)
    }
}
impl Diff for Trait {
    type Diff = Option<TraitDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let vis_diff = self.vis.diff_with(&other.vis);
        let generics_diff = self.generics.diff_with(&other.generics);
        let items_diff = {
            // TODO: only supports functions
            if self.items == other.items {
                return None;
            }

            let self_fn_items: Vec<_> = self
                .items
                .iter()
                .filter_map(|i| match i {
                    TraitItem::Fn(func) => Some(func.clone()),
                    _ => None,
                })
                .collect();
            let other_fn_items: Vec<_> = other
                .items
                .iter()
                .filter_map(|i| match i {
                    TraitItem::Fn(func) => Some(func.clone()),
                    _ => None,
                })
                .collect();

            let self_items_fns = Functions::new_trait(self_fn_items, self.source.clone());
            let other_items_fns = Functions::new_trait(other_fn_items, other.source.clone());

            let fns_diff = self_items_fns.diff_with(&other_items_fns);
            if fns_diff.is_empty() {
                None
            } else {
                Some(TraitItemsDiff { fns_diff })
            }
        };

        // it's possible for a trait to be non-equal and yet the diffs don't contain
        // anything we're interested in tracking.
        if vis_diff.is_none() && generics_diff.is_none() && items_diff.is_none() {
            return None;
        }

        let (old_src_map, new_src_map) = collect_src_maps!(vis_diff, items_diff, generics_diff,);

        let diff = TraitDiff {
            name: self.name.clone(),
            change: Change::Modified,
            old: Some(self.original.clone()),
            new: Some(other.original.clone()),
            old_src: Some(self.source.clone()),
            new_src: Some(other.source.clone()),
            old_src_map,
            new_src_map,
            vis_diff,
            items_diff,
            generics_diff,
        };

        Some(diff)
    }
}

pub struct TraitsDiff {
    diffs: Vec<TraitDiff>,
}
impl TraitsDiff {
    pub(crate) fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }
}
impl View for TraitsDiff {
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

#[derive(Debug, Default)]
#[allow(unused)]
pub struct TraitDiff {
    name: String,
    change: Change,
    old: Option<ItemTrait>,
    new: Option<ItemTrait>,
    old_src: Option<SourceFile>,
    new_src: Option<SourceFile>,
    old_src_map: Vec<Range<usize>>,
    new_src_map: Vec<Range<usize>>,
    vis_diff: Option<VisDiff>,
    items_diff: Option<TraitItemsDiff>,
    generics_diff: Option<GenericsDiff>,
}
impl View for TraitDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        if let Change::Existence(ex) = self.change {
            return collect_existence_diff_changes(&self.old, &self.new, ex);
        }

        let old_diff = collect_trait_diff_changes(
            self.old.as_ref().unwrap(),
            &self.old_src.as_ref().unwrap().0.as_bytes(),
            &self.old_src_map,
            &self.items_diff,
            ExistenceChange::Deleted,
        );

        let new_diff = collect_trait_diff_changes(
            self.new.as_ref().unwrap(),
            &self.new_src.as_ref().unwrap().0.as_bytes(),
            &self.new_src_map,
            &self.items_diff,
            ExistenceChange::Added,
        );

        ViewableDiffs::new(vec![ViewableDiff {
            old: Some(old_diff),
            new: Some(new_diff),
        }])
    }
}

// #[derive(Debug)]
// pub struct TraitItemDiff {
//     change: Change,
//     trait_fn_diff: Option<FunctionDiff>,
//     old: Option<TraitItem>,
//     new: Option<TraitItem>,
// }
// impl ByteRange for TraitItemDiff {
//     fn old_ranges(&self) -> Vec<Range<usize>> {
//         let mut ranges = vec![];

//         if let Some(old) = &self.old {
//             ranges.push(old.span().byte_range());
//         }

//         ranges
//     }

//     fn new_ranges(&self) -> Vec<Range<usize>> {
//         let mut ranges = vec![];

//         if let Some(new) = &self.new {
//             ranges.push(new.span().byte_range());
//         }

//         ranges
//     }
// }

// impl ByteRange for Vec<TraitItemDiff> {
//     fn old_ranges(&self) -> Vec<Range<usize>> {
//         let mut ranges = vec![];
//         for diff in self.iter() {
//             ranges.extend(diff.old_ranges());
//         }

//         ranges
//     }

//     fn new_ranges(&self) -> Vec<Range<usize>> {
//         let mut ranges = vec![];
//         for diff in self.iter() {
//             ranges.extend(diff.new_ranges());
//         }

//         ranges
//     }
// }

fn collect_existence_diff_changes(
    old: &Option<ItemTrait>,
    new: &Option<ItemTrait>,
    ex: ExistenceChange,
) -> ViewableDiffs {
    let trait_ = match (&old, &new) {
        (Some(_), Some(_)) => panic!("old and new traits were both Some"),
        (None, None) => panic!("old and new traits were both None"),
        (Some(trait_), None) | (None, Some(trait_)) => trait_,
    };

    let source = trait_.span().source_text().expect(NO_SRC_ERROR);
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

fn collect_trait_diff_changes(
    trait_: &ItemTrait,
    source_code: &[u8],
    source_map: &[Range<usize>],
    item_diffs: &Option<TraitItemsDiff>,
    ex: ExistenceChange,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let source_range = trait_.span().byte_range();
    let decl_start = source_range.start;
    let sig_end = end_index_of_signature(trait_);
    let items_end = end_index_of_items(trait_, source_code);

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

                diff_changes.push((Some(ex), code));

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

    if let Some(item_diffs) = item_diffs {
        let (get_orig_item, get_sub_diff): (
            Box<dyn Fn(&FunctionDiff) -> Option<&Function>>,
            Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
        );
        match ex {
            ExistenceChange::Added => {
                get_orig_item = Box::new(|fd: &FunctionDiff| fd.new().as_ref());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.new);
            }
            ExistenceChange::Deleted => {
                get_orig_item = Box::new(|fd: &FunctionDiff| fd.old().as_ref());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.old);
            }
        };
        let diffs_as_changes = collect_item_diff_changes(
            source_code,
            &source_range,
            sig_end,
            item_diffs,
            get_orig_item,
            get_sub_diff,
        );
        diff_changes.extend(diffs_as_changes);
    }

    // collect remaining whitespace and closing ')' or '}'
    let code = String::from_utf8(source_code[items_end..source_range.end].to_vec())
        .expect("Off a code boundary");

    diff_changes.push((None, Code(code)));
    diff_changes
}

/// Converts field diffs into spans of code indicating the changes, if any.
fn collect_item_diff_changes(
    // The full source code for the file we're parsing
    source_code: &[u8],
    // The byte range in the source code of the trait we're parsing
    trait_range: &Range<usize>,
    // The index at which the signature ends
    sig_end: usize,
    // The field diffs for the file we're parsing
    tids: &TraitItemsDiff,
    // How to get the original TraitItem from a TraitItemDiff (old or new method)
    get_original_item: Box<dyn Fn(&FunctionDiff) -> Option<&Function>>,
    // How to get sub diffs from a given viewable diff (old or new field)
    get_sub_diffs: Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let mut diffs = Vec::new();
    let mut i = sig_end;

    while i < trait_range.end {
        let maybe_item_diff = tids.diffs().iter().find(|d| {
            get_original_item(d)
                .as_ref()
                .map(|item| item.original().span().byte_range().contains(&i))
                .unwrap_or(false)
        });
        match maybe_item_diff {
            Some(id) => {
                // Going to walk backwards and get all preceding whitespace until a newline or a character
                let item_diff_range = get_original_item(id)
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

                // TODO: this omits commas between fields (applies to variants and traits, too)
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

fn end_index_of_signature(trait_: &ItemTrait) -> usize {
    trait_.brace_token.span.span().byte_range().start + 1
}

fn end_index_of_items(trait_: &ItemTrait, source_code: &[u8]) -> usize {
    let mut index_before_close_brace = trait_.span().byte_range().end - 2;
    while index_before_close_brace > 0
        && source_code[index_before_close_brace].is_ascii_whitespace()
    {
        index_before_close_brace -= 1;
    }

    index_before_close_brace += 1; // we want 1 index beyond the last char we ended at
    index_before_close_brace
}

#[derive(Debug)]
pub struct TraitItemsDiff {
    fns_diff: FunctionsDiff,
}
impl TraitItemsDiff {
    pub fn diffs(&self) -> &[FunctionDiff] {
        &self.fns_diff.diffs()
    }
}
impl ByteRange for TraitItemsDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        self.fns_diff.old_ranges()
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        self.fns_diff.new_ranges()
    }
}
