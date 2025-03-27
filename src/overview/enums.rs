use std::{fmt::Display, ops::Deref};

use quote::ToTokens;
use syn::{Item, ItemEnum};

use crate::{format_as_columns, get_source, Change, Diff, ExistenceChange, Vis, VisDiff};

use super::{
    generics::{Generics, GenericsDiff},
    variants::{VariantDiffs, Variants},
};

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
                        new: None,
                        vis_diff: None,
                        variants_diff: None,
                        generics_diff: None,
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
                    old: None,
                    new: Some(enum_.original.clone()),
                    vis_diff: None,
                    variants_diff: None,
                    generics_diff: None,
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
    vis: Vis,
    variants: Variants,
    generics: Generics,
    original: ItemEnum,
}
impl Enum {
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

        Some(EnumDiff {
            name: self_name.to_string(),
            change: Change::Modified,
            old: Some(self.original.clone()),
            new: Some(other.original.clone()),
            vis_diff,
            variants_diff,
            generics_diff,
        })
    }
}
impl From<ItemEnum> for Enum {
    fn from(s: ItemEnum) -> Self {
        let original = s.clone();
        let vis = s.vis.into();
        let name = s.ident.to_string();
        let variants: Vec<_> = s.variants.into_iter().collect();
        let variants = Variants::from(variants);
        let generics = Generics::from(s.generics.clone());

        Self {
            name,
            vis,
            variants,
            generics,
            original,
        }
    }
}
impl Display for Enum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} struct {}", self.vis.as_str(), self.name)
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
impl Display for EnumsDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.enums.is_empty() {
            return writeln!(f, "(no changes)");
        }

        for diff in self.enums.iter() {
            writeln!(f, "{diff}")?;
        }

        Ok(())
    }
}

/// A diff between two enum declarations.
#[derive(Debug)]
pub struct EnumDiff {
    name: String,
    change: Change,
    // present if the enum was deleted or modified
    old: Option<ItemEnum>,
    // present if the enum was added or modified
    new: Option<ItemEnum>,
    vis_diff: Option<VisDiff>,
    variants_diff: Option<VariantDiffs>,
    generics_diff: Option<GenericsDiff>,
}
impl Display for EnumDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Change::Existence(ex) = self.change {
            let e = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new enums were both Some"),
                (None, None) => panic!("old and new enums were both None"),
                (Some(e), None) | (None, Some(e)) => e,
            };

            let source = get_source(vec![Item::Enum(e.clone())]);
            return write!(f, "{ex} {source}");
        }

        let mut left_column = Vec::new();
        let mut right_column = Vec::new();

        // old and new neum declarations
        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = get_source(vec![Item::Enum(old.clone())]);
        let new_source = get_source(vec![Item::Enum(new.clone())]);
        left_column.push(old_source);
        right_column.push(new_source);

        // old and new visibility modifiers, if any
        if let Some(vd) = &self.vis_diff {
            left_column.push("\nvisibility:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", vd.old));
            right_column.push(format!("+ {}", vd.new));
        }

        if let Some(vd) = &self.variants_diff {
            let mut old_variants = Vec::new();
            let mut new_variants = Vec::new();

            for vd in vd.diffs() {
                match vd.change() {
                    Change::Existence(ex) => {
                        // variant was added or deleted wholesale
                        match ex {
                            ExistenceChange::Deleted => {
                                let variant_source =
                                    vd.old().unwrap().to_token_stream().to_string();
                                old_variants.push(format!("- {variant_source}",))
                            }
                            ExistenceChange::Added => {
                                let variant_source =
                                    vd.new().unwrap().to_token_stream().to_string();
                                new_variants.push(format!("+ {variant_source}",))
                            }
                        }
                    }
                    Change::Modified => {
                        // variant was modified
                        let old_variant_source = vd.old().unwrap().to_token_stream().to_string();
                        let new_variant_source = vd.new().unwrap().to_token_stream().to_string();
                        old_variants.push(format!("- {old_variant_source}",));
                        new_variants.push(format!("+ {new_variant_source}",));
                    }
                }
            }

            left_column.push("\nvariants:".to_string());
            right_column.push(String::new());
            left_column.push(old_variants.join("\n"));
            right_column.push(new_variants.join("\n"));
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

        let formatted_output = format_as_columns(&left_column, &right_column);
        write!(f, "{formatted_output}")
    }
}
