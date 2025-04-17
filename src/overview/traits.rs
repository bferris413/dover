use std::{
    fmt::{Display, Write},
    ops::Deref,
};

use quote::ToTokens;
use syn::{spanned::Spanned, Item, ItemTrait, TraitItem, Visibility};

use crate::{get_source, Change, Diff, ExistenceChange, SourceFile, VisDiff};

use super::generics::{Generics, GenericsDiff};

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

pub struct TraitsDiff {
    diffs: Vec<TraitDiff>,
}
impl TraitsDiff {
    pub(crate) fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }
}
impl Display for TraitsDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ex_diffs = self
            .diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Existence(_)));

        let (mut left_col, mut right_col) = (String::new(), String::new());
        let mut any_ex_diffs = false;
        for diff in ex_diffs {
            any_ex_diffs = true;
            match diff.change {
                Change::Existence(ExistenceChange::Added) => {
                    write!(right_col, "{diff}")?;
                }
                Change::Existence(ExistenceChange::Deleted) => {
                    write!(left_col, "{diff}")?;
                }
                _ => {
                    unreachable!()
                }
            }
        }
        if any_ex_diffs {
            let output = crate::format_as_columns(&vec![left_col], &vec![right_col]);
            writeln!(f, "{output}")?;
        }

        let mod_diffs = self
            .diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for diff in mod_diffs {
            writeln!(f, "{diff}")?;
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
            let source = crate::get_source(vec![Item::Trait(t.clone())])
                .lines()
                .map(|line| format!("{ex} {line}"))
                .collect::<Vec<String>>()
                .join("\n");
            return write!(f, "{source}");
        }

        let mut left_column = Vec::new();
        let mut right_column = Vec::new();

        // old and new trait declarations
        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = crate::get_source(vec![Item::Trait(old.clone())])
            .lines()
            .map(|line| format!("~ {line}"))
            .collect::<Vec<String>>()
            .join("\n");
        let new_source = crate::get_source(vec![Item::Trait(new.clone())])
            .lines()
            .map(|line| format!("~ {line}"))
            .collect::<Vec<String>>()
            .join("\n");
        left_column.push(old_source);
        right_column.push(new_source);

        // old and new visibility modifiers, if any
        if let Some(vd) = &self.vis_diff {
            left_column.push("\nvisibility:".to_string());
            right_column.push(String::new());
            let old_vis = vd.old.span().source_text();
            let new_vis = vd.new.span().source_text();
            left_column.push(format!("- {}", old_vis.as_deref().unwrap_or("(none)")));
            right_column.push(format!("- {}", new_vis.as_deref().unwrap_or("(none)")));
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
                            .span()
                            .source_text()
                            .expect(NO_SRC_ERROR);
                        let new_item_source = item_diff
                            .new
                            .as_ref()
                            .unwrap()
                            .span()
                            .source_text()
                            .expect(NO_SRC_ERROR);
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
                    let param_source = pd.param().span().source_text().expect(NO_SRC_ERROR);
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
                        let where_clause_source = wd
                            .where_clause()
                            .unwrap()
                            .span()
                            .source_text()
                            .expect(NO_SRC_ERROR);
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
                            let predicate_source = pred_diff
                                .predicate()
                                .span()
                                .source_text()
                                .expect(NO_SRC_ERROR);
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
        write!(f, "{formatted_output}")
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
