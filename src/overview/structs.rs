use std::{fmt::Display, ops::Deref};

use quote::ToTokens;
use syn::{File, Item, ItemStruct, Visibility};

use crate::{Change, Diff, ExistenceChange};

use super::{
    fields::{Field, Fields, FieldsDiff},
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
        let fields = s.fields.into_iter().map(Field::from).collect();
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
    #[allow(unused)]
    fields_diff: Option<FieldsDiff>,
    #[allow(unused)]
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

        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = get_source(vec![Item::Struct(old.clone())]);
        let new_source = get_source(vec![Item::Struct(new.clone())]);
        writeln!(f, "{} {old_source}", self.change)?;
        writeln!(f, "{} {new_source}", self.change)?;

        // if let Some(vd) = &self.vis_diff {
        //     writeln!(f, "vis:")?;
        //     writeln!(f, "- {}", vd.old)?;
        //     writeln!(f, "+ {}", vd.new)?;
        // }

        Ok(())
    }
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
