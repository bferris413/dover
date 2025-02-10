use std::{fmt::Display, ops::Deref};

use syn::{ItemStruct, Visibility};

use crate::{Change, Diff, ExistenceChange};

use super::fields::{Field, Fields, FieldsDiff};

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
                    let sdiff = StructDiff {
                        name: struct_.name().to_string(),
                        change: Change::Existence(ExistenceChange::Deleted),
                        struct_: Some(struct_.clone()),
                        vis_diff: None,
                        fields_diff: None,
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
                    struct_: Some(struct_.clone()),
                    vis_diff: None,
                    fields_diff: None,
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
        Some(StructDiff {
            name: self_name.to_string(),
            change: Change::Modified,
            struct_: None,
            vis_diff,
            fields_diff,
        })
    }
}
impl From<ItemStruct> for Struct {
    fn from(s: ItemStruct) -> Self {
        let vis = s.vis.into();
        let name = s.ident.to_string();
        let fields = s.fields.into_iter().map(Field::from).collect();
        let fields = Fields(fields);
        Self { name, vis, fields }
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
            writeln!(f, "{diff:#?}")?;
        }

        Ok(())
    }
}

/// A diff between two struct declarations.
#[derive(Debug)]
pub struct StructDiff {
    name: String,
    change: Change,
    // only populated if change is added or deleted
    struct_: Option<Struct>,
    vis_diff: Option<VisDiff>,
    #[allow(unused)]
    fields_diff: Option<FieldsDiff>,
}
impl Display for StructDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Change::Existence(ex) = self.change {
            return write!(f, "{ex} {}", self.struct_.as_ref().unwrap());
        }

        writeln!(f, "{} struct {}:", self.change, self.name)?;
        if let Some(vd) = &self.vis_diff {
            writeln!(f, "vis:")?;
            writeln!(f, "- {}", vd.old)?;
            writeln!(f, "+ {}", vd.new)?;
        }

        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct VisDiff {
    pub old: Vis,
    pub new: Vis,
}
