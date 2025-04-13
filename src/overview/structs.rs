use std::{
    fmt::{Display, Write},
    ops::Deref,
};

use quote::ToTokens;
use syn::{Item, ItemStruct};

use crate::{Change, Diff, ExistenceChange, Vis, VisDiff};

use super::{
    fields::{Fields, FieldsDiff},
    generics::{Generics, GenericsDiff},
};

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
                        new: None,
                        vis_diff: None,
                        fields_diff: None,
                        generics_diff: None,
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
                    old: None,
                    new: Some(struct_.original.clone()),
                    vis_diff: None,
                    fields_diff: None,
                    generics_diff: None,
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
    vis: Vis,
    fields: Fields,
    generics: Generics,
    original: ItemStruct,
}
impl Struct {
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

        Some(StructDiff {
            name: self_name.to_string(),
            change: Change::Modified,
            old: Some(self.original.clone()),
            new: Some(other.original.clone()),
            vis_diff,
            fields_diff,
            generics_diff,
        })
    }
}
impl From<ItemStruct> for Struct {
    fn from(s: ItemStruct) -> Self {
        let original = s.clone();
        let vis: Vis = s.vis.into();
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
        }
    }
}
impl Display for Struct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} struct {}", self.vis.as_str(), self.name)
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
impl Display for StructsDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.structs.is_empty() {
            return writeln!(f, "(no changes)");
        }

        let ex_diffs = self
            .structs
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
            .structs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for diff in mod_diffs {
            writeln!(f, "{diff}")?;
        }

        Ok(())
    }
}

/// A diff between two struct declarations.
#[derive(Debug)]
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
}
impl Display for StructDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Change::Existence(ex) = self.change {
            let s = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new structs were both Some"),
                (None, None) => panic!("old and new structs were both None"),
                (Some(s), None) | (None, Some(s)) => s,
            };

            let source = crate::get_source(vec![Item::Struct(s.clone())])
                .lines()
                .map(|line| format!("{ex} {line}"))
                .collect::<Vec<String>>()
                .join("\n");
            return write!(f, "{source}");
        }

        let mut left_column = Vec::new();
        let mut right_column = Vec::new();

        // old and new struct declarations
        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = crate::get_source(vec![Item::Struct(old.clone())])
            .lines()
            .map(|line| format!("~ {line}"))
            .collect::<Vec<String>>()
            .join("\n");
        let new_source = crate::get_source(vec![Item::Struct(new.clone())])
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
            left_column.push(format!("- {}", vd.old));
            right_column.push(format!("+ {}", vd.new));
        }

        if let Some(fd) = &self.fields_diff {
            let mut old_fields = Vec::new();
            let mut new_fields = Vec::new();

            for fd in fd.diffs() {
                if let Some(fd) = fd {
                    match fd.change() {
                        Change::Existence(ex) => {
                            // field was added or deleted wholesale
                            match ex {
                                ExistenceChange::Deleted => {
                                    let field_source =
                                        fd.old().unwrap().to_token_stream().to_string();
                                    old_fields.push(format!("- {field_source}",))
                                }
                                ExistenceChange::Added => {
                                    let field_source =
                                        fd.new().unwrap().to_token_stream().to_string();
                                    new_fields.push(format!("+ {field_source}",))
                                }
                            }
                        }
                        Change::Modified => {
                            // field was modified
                            let old_field_source = fd.old().unwrap().to_token_stream().to_string();
                            let new_field_source = fd.new().unwrap().to_token_stream().to_string();
                            old_fields.push(format!("- {old_field_source}",));
                            new_fields.push(format!("+ {new_field_source}",));
                        }
                    }
                }
            }

            left_column.push("\nfields:".to_string());
            right_column.push(String::new());
            left_column.push(old_fields.join("\n"));
            right_column.push(new_fields.join("\n"));
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
        write!(f, "{formatted_output}")
    }
}
