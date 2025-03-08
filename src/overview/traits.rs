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
        if let Change::Existence(ex) = self.change {
            let t = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new traits were both Some"),
                (None, None) => panic!("old and new traits were both None"),
                (Some(t), None) | (None, Some(t)) => t,
            };
            let source = crate::get_source(vec![Item::Trait(t.clone())]);
            return write!(f, "{ex} {source}");
        }

        let mut left_column = Vec::new();
        let mut right_column = Vec::new();

        // old and new trait declarations
        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = crate::get_source(vec![Item::Trait(old.clone())]);
        let new_source = crate::get_source(vec![Item::Trait(new.clone())]);
        left_column.push(old_source);
        right_column.push(new_source);

        // old and new visibility modifiers, if any
        if let Some(vd) = &self.vis_diff {
            left_column.push("\nvisibility:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", vd.old));
            right_column.push(format!("+ {}", vd.new));
        }

        if let Some(items_diff) = &self.items_diff {
            let mut old_items = Vec::new();
            let mut new_items = Vec::new();

            for item_diff in items_diff {
                match item_diff.change {
                    Change::Existence(ex) => {
                        // item was added or deleted wholesale
                        match ex {
                            ExistenceChange::Deleted => {
                                let item_source = item_diff
                                    .old
                                    .as_ref()
                                    .unwrap()
                                    .to_token_stream()
                                    .to_string();
                                old_items.push(format!("- {item_source}",))
                            }
                            ExistenceChange::Added => {
                                let item_source = item_diff
                                    .new
                                    .as_ref()
                                    .unwrap()
                                    .to_token_stream()
                                    .to_string();
                                new_items.push(format!("+ {item_source}",))
                            }
                        }
                    }
                    Change::Modified => {
                        // item was modified
                        let old_item_source = item_diff
                            .old
                            .as_ref()
                            .unwrap()
                            .to_token_stream()
                            .to_string();
                        let new_item_source = item_diff
                            .new
                            .as_ref()
                            .unwrap()
                            .to_token_stream()
                            .to_string();
                        old_items.push(format!("- {old_item_source}",));
                        new_items.push(format!("+ {new_item_source}",));
                    }
                }
            }

            left_column.push("\nitems:".to_string());
            right_column.push(String::new());
            left_column.push(old_items.join("\n"));
            right_column.push(new_items.join("\n"));
        }

        // old and new generics, if any
        if let Some(gd) = &self.generics_diff {
            // generic param diff, if any
            if let Some(pd) = gd.params_diff() {
                let mut old_params = Vec::new();
                let mut new_params = Vec::new();

                for pd in pd.iter() {
                    let param_source = pd.param().unwrap().to_token_stream().to_string();
                    // let param_source = get_source(vec![Item::Verbatim(param_tokens)]);
                    match pd.change() {
                        ExistenceChange::Deleted => old_params.push(format!("- {param_source}",)),
                        ExistenceChange::Added => new_params.push(format!("+ {param_source}",)),
                    }
                }

                left_column.push("\ngeneric parameters:".to_string());
                right_column.push(String::new());
                left_column.push(old_params.join("\n"));
                right_column.push(new_params.join("\n"));
            }

            // where clause diff, if any
            if let Some(wd) = gd.where_diff() {
                left_column.push("\nwhere clause:".to_string());
                right_column.push(String::new());
                match wd.change() {
                    Change::Existence(ex) => {
                        // where clause was added or deleted wholesale
                        let where_clause_source =
                            wd.where_clause().unwrap().to_token_stream().to_string();
                        // let where_clause_source = get_source(vec![Item::Verbatim(where_clause)]);
                        match ex {
                            ExistenceChange::Deleted => {
                                left_column.push(format!("- {where_clause_source}"));
                                right_column.push(String::new());
                            }
                            ExistenceChange::Added => {
                                right_column.push(format!("+ {where_clause_source}"));
                                left_column.push(String::new());
                            }
                        }
                    }
                    Change::Modified => {
                        // where clause predicates were added or deleted
                        let predicate_diffs = wd.predicates().unwrap();
                        let mut old_predicates = Vec::new();
                        let mut new_predicates = Vec::new();

                        for pred_diff in predicate_diffs.iter() {
                            let predicate_source =
                                pred_diff.predicate().unwrap().to_token_stream().to_string();
                            // let predicate_source =
                            //     get_source(vec![Item::Verbatim(predicate_tokens)]);
                            match pred_diff.change() {
                                ExistenceChange::Deleted => {
                                    old_predicates.push(format!("- {predicate_source}",))
                                }
                                ExistenceChange::Added => {
                                    new_predicates.push(format!("+ {predicate_source}",))
                                }
                            }
                        }

                        left_column.push(old_predicates.join("\n"));
                        right_column.push(new_predicates.join("\n"));
                    }
                }
            }
        }

        // field diff, if any

        let formatted_output = crate::format_as_columns(&left_column, &right_column);
        writeln!(f, "{formatted_output}")
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
