use std::{fmt::Display, ops::Deref};

use quote::ToTokens;
use syn::{File, Item, ItemStruct, Visibility};

use crate::{Change, Diff, ExistenceChange};

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
        let vis = s.vis.into();
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Vis {
    Public,
    Restricted,
    Inherited,
}
impl Vis {
    pub fn as_str(&self) -> &'static str {
        match self {
            Vis::Public => "pub",
            Vis::Restricted => "pub(..)",
            Vis::Inherited => "(none)",
        }
    }
}
impl Diff for Vis {
    type Diff = Option<VisDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        Some(VisDiff {
            old: *self,
            new: *other,
        })
    }
}
impl Display for Vis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl From<Visibility> for Vis {
    fn from(vis: Visibility) -> Self {
        match vis {
            Visibility::Public(_) => Vis::Public,
            Visibility::Restricted(_) => Vis::Restricted,
            Visibility::Inherited => Vis::Inherited,
        }
    }
}

/// A collection of diffs for `struct` declarations.
pub struct StructsDiff {
    structs: Vec<StructDiff>,
}
impl Display for StructsDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.structs.is_empty() {
            return writeln!(f, "(no changes)");
        }

        for diff in self.structs.iter() {
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

            let source = get_source(vec![Item::Struct(s.clone())]);
            return write!(f, "{ex} {source}");
        }

        let mut left_column = Vec::new();
        let mut right_column = Vec::new();

        // old and new struct declarations
        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = get_source(vec![Item::Struct(old.clone())]);
        let new_source = get_source(vec![Item::Struct(new.clone())]);
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

        let formatted_output = format_as_columns(&left_column, &right_column);
        writeln!(f, "{formatted_output}")
    }
}

fn format_as_columns(left: &Vec<String>, right: &Vec<String>) -> String {
    // Each string is a section of the struct diff. We expect there to be an equal number
    // of sections in the left and right columns, even though the number of lines per section
    // may be different.
    assert_eq!(left.len(), right.len());

    // Get the maximum width of the left column across all lines within each section
    // so we can align the right column.
    let max_width = left
        .iter()
        .map(|s| s.lines().map(|s| s.len()).max().unwrap_or(0))
        .max()
        .unwrap()
        .max(50);

    let left_right = left.iter().zip(right.iter());
    let mut formatted_output = String::new();

    for (left, right) in left_right {
        let mut left_lines = left.lines().collect::<Vec<_>>();
        let mut right_lines = right.lines().collect::<Vec<_>>();

        // Pad the left and right columns with empty lines so they have the same number of lines
        // (zip short circuits when one of the iterators is exhausted)
        while left_lines.len() < right_lines.len() {
            left_lines.push("");
        }
        while right_lines.len() < left_lines.len() {
            right_lines.push("");
        }

        for (left_line, right_line) in left_lines.iter().zip(right_lines.iter()) {
            formatted_output.push_str(&format!(
                "{:<width$} {}",
                left_line,
                right_line,
                width = max_width
            ));
            formatted_output.push('\n');
        }
    }

    formatted_output
}

#[derive(Debug, Eq, PartialEq)]
pub struct VisDiff {
    pub old: Vis,
    pub new: Vis,
}

fn get_source(items: Vec<Item>) -> String {
    let syn_file = File {
        items,
        shebang: None,
        attrs: vec![],
    };

    prettyplease::unparse(&syn_file)
}
