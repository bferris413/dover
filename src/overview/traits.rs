use std::{
    fmt::Display,
    ops::{Deref, Range},
};

use syn::{spanned::Spanned, Item, ItemTrait, TraitItem, Visibility};

use super::generics::{Generics, GenericsDiff};
use crate::{
    collect_src_maps, get_source, ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View,
    ViewableDiff, ViewableDiffs, VisDiff,
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
        let items_diff = self.items.diff_with(&other.items);

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
    items_diff: Option<Vec<TraitItemDiff>>,
    generics_diff: Option<GenericsDiff>,
}
impl View for TraitDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        if let Change::Existence(ex) = self.change {
            let _struct = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new traits were both Some"),
                (None, None) => panic!("old and new traits were both None"),
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
        // if we want to remove the block we'd do it here
        let old_range = old.span().byte_range();
        // let old_end = old.original_fn.block.span().byte_range().start;
        // let old_range = old_start..old_end;

        let mut i = old_range.start;
        let mut src_i = 0;
        let mut old_diff = Vec::new();

        while i < old_range.end {
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
                    while i < old_range.end {
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
        // if we want to remove the block we'd do it here
        let new_range = new.span().byte_range();
        // let new_end = new.original_fn.block.span().byte_range().start;
        // let new_range = new_start..new_end;

        let mut i = new_range.start;
        let mut src_i = 0;
        let mut new_diff = Vec::new();

        while i < new_range.end {
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
                    while i < new_range.end {
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

impl Diff for TraitItem {
    type Diff = Option<TraitItemDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }
        let change = Change::Modified;
        let old = self.clone();
        let new = other.clone();

        Some(TraitItemDiff {
            change,
            old: Some(old),
            new: Some(new),
        })
    }
}

impl Diff for Vec<TraitItem> {
    type Diff = Option<Vec<TraitItemDiff>>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let mut item_diffs = Vec::new();

        // extremely coarse. eventually we want to diff items themselves, but for
        // now we just use full equality with added/removed changes (no modifications)
        for old_item in self.iter() {
            if !other.contains(old_item) {
                let change = Change::Existence(ExistenceChange::Deleted);
                let diff = TraitItemDiff {
                    change,
                    old: Some(old_item.clone()),
                    new: None,
                };
                item_diffs.push(diff);
            }
        }

        for new_item in other.iter() {
            if !self.contains(new_item) {
                let change = Change::Existence(ExistenceChange::Added);
                let diff = TraitItemDiff {
                    change,
                    old: None,
                    new: Some(new_item.clone()),
                };
                item_diffs.push(diff);
            }
        }

        if item_diffs.is_empty() {
            return None;
        } else {
            Some(item_diffs)
        }
    }
}

#[derive(Debug)]
pub struct TraitItemDiff {
    change: Change,
    old: Option<TraitItem>,
    new: Option<TraitItem>,
}
impl ByteRange for TraitItemDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = vec![];

        if let Some(old) = &self.old {
            ranges.push(old.span().byte_range());
        }

        ranges
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = vec![];

        if let Some(new) = &self.new {
            ranges.push(new.span().byte_range());
        }

        ranges
    }
}

impl ByteRange for Vec<TraitItemDiff> {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = vec![];
        for diff in self.iter() {
            ranges.extend(diff.old_ranges());
        }

        ranges
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = vec![];
        for diff in self.iter() {
            ranges.extend(diff.new_ranges());
        }

        ranges
    }
}
