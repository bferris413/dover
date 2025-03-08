use std::{fmt::Display, ops::Deref};

use quote::ToTokens;
use syn::{Item, ItemTrait, TraitItem};

use crate::{Change, Diff, ExistenceChange, Vis, VisDiff};

use super::generics::{Generics, GenericsDiff};

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
                        new: None,
                        vis_diff: None,
                        items_diff: None,
                        generics_diff: None,
                    };
                    trait_diffs.push(sdiff);
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for struct_ in &other.0 {
            if let Err(_e) = self.0.binary_search_by(|s| s.name().cmp(struct_.name())) {
                let sdiff = TraitDiff {
                    name: struct_.name().to_string(),
                    change: Change::Existence(ExistenceChange::Added),
                    old: None,
                    new: Some(struct_.original.clone()),
                    vis_diff: None,
                    items_diff: None,
                    generics_diff: None,
                };
                trait_diffs.push(sdiff);
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
    vis: Vis,
    generics: Generics,
    items: Vec<TraitItem>,
    original: ItemTrait,
}
impl Trait {
    pub fn name(&self) -> &str {
        &self.name
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

        let diff = TraitDiff {
            name: self.name.clone(),
            change: Change::Modified,
            old: Some(self.original.clone()),
            new: Some(other.original.clone()),
            vis_diff,
            items_diff,
            generics_diff,
        };

        Some(diff)
    }
}
impl From<ItemTrait> for Trait {
    fn from(t: ItemTrait) -> Self {
        let original = t.clone();
        let name = t.ident.to_string();
        let vis = t.vis.into();
        let generics = Generics::from(t.generics);
        let items = t.items;
        Trait {
            name,
            vis,
            generics,
            items,
            original,
        }
    }
}

pub struct TraitsDiff {
    diffs: Vec<TraitDiff>,
}
impl Display for TraitsDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for diff in &self.diffs {
            writeln!(f, "{}", diff)?;
        }
        Ok(())
    }
}

pub struct TraitDiff {
    pub name: String,
    pub change: Change,
    pub old: Option<ItemTrait>,
    pub new: Option<ItemTrait>,
    pub vis_diff: Option<VisDiff>,
    pub items_diff: Option<Vec<TraitItemDiff>>,
    pub generics_diff: Option<GenericsDiff>,
}
impl Display for TraitDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Change::*;
        match self.change {
            Existence(ExistenceChange::Added) => {
                let source = crate::get_source(vec![Item::Trait(self.new.clone().unwrap())]);
                writeln!(f, "+ {source}")
            }
            Existence(ExistenceChange::Deleted) => {
                let source = crate::get_source(vec![Item::Trait(self.old.clone().unwrap())]);
                writeln!(f, "- {source}")
            }
            Modified => {
                todo!()
            }
        }
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

pub struct TraitItemDiff {
    change: Change,
    old: Option<TraitItem>,
    new: Option<TraitItem>,
}
impl Display for TraitItemDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Change::*;
        match self.change {
            Existence(ExistenceChange::Added) => {
                let tokens = self.new.to_token_stream();
                let source = crate::get_source(vec![Item::Verbatim(tokens)]);
                writeln!(f, "+ {source}")
            }
            Existence(ExistenceChange::Deleted) => {
                let tokens = self.old.to_token_stream();
                let source = crate::get_source(vec![Item::Verbatim(tokens)]);
                writeln!(f, "- {source}")
            }
            _ => unreachable!(),
        }
    }
}
