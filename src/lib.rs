use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use quote::ToTokens;
use syn::ItemUse;
use syn::{File, Item, ItemFn};

use overview::structs::{Struct, Structs, StructsDiff};
use overview::uses::{self, Uses, UsesDiff};

mod git;
mod overview;

pub use git::{get_changed_files, Change as GitChange, ChangedFile};

/// Diff an item with another and return the result.
pub trait Diff {
    type Diff;
    fn diff_with(&self, other: &Self) -> Self::Diff;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Change {
    Modified,
    Existence(ExistenceChange),
}
impl Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Change::Modified => write!(f, "~"),
            Change::Existence(ex) => write!(f, "{ex}"),
        }
    }
}
impl From<ExistenceChange> for Change {
    fn from(existence: ExistenceChange) -> Self {
        Change::Existence(existence)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Copy)]
pub enum ExistenceChange {
    Added,
    Deleted,
}
impl Display for ExistenceChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExistenceChange::Added => write!(f, "+"),
            ExistenceChange::Deleted => write!(f, "-"),
        }
    }
}

fn get_overview(path: PathBuf, contents: String) -> Result<Overview> {
    let file: File = syn::parse_file(&contents).context("Error parsing {path}")?;
    let mut use_statements = Vec::new();
    let mut functions = Vec::new();
    let mut structs = Vec::new();

    for item in file.items {
        match item {
            Item::Use(item_use @ ItemUse { .. }) => {
                use_statements.push(item_use);
            }
            Item::Fn(item_fn @ ItemFn { .. }) => {
                functions.push(item_fn);
            }
            Item::Struct(item_struct) => {
                structs.push(item_struct);
            }
            _ => {}
        }
    }

    let structs = structs.into_iter().map(Struct::from).collect();
    let structs = Structs::from(structs);

    // dbg!(&functions);
    for func in functions.into_iter() {
        let func_sig = func.sig.into_token_stream();
        let func_str = func_sig.to_string();
        println!("{func_str}");
    }

    let mut use_paths = Vec::new();
    for r#use in use_statements.iter() {
        // let visibility = import.vis;
        let tree = &r#use.tree;

        let paths = uses::get_paths_from_usetree(tree);
        use_paths.extend(paths.into_iter());
    }

    let overview = Overview {
        path,
        uses: Uses::from(use_paths),
        structs,
    };
    Ok(overview)
}

#[derive(Debug)]
pub struct Overview {
    path: PathBuf,
    uses: Uses,
    structs: Structs,
}
impl Overview {
    pub fn uses(&self) -> &Uses {
        &self.uses
    }
}
impl TryFrom<(PathBuf, String)> for Overview {
    type Error = anyhow::Error;

    fn try_from((path, contents): (PathBuf, String)) -> std::result::Result<Self, Self::Error> {
        get_overview(path, contents)
    }
}
impl TryFrom<PathBuf> for Overview {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> std::result::Result<Self, Self::Error> {
        let contents = fs::read_to_string(&path).context("Error reading file at {path}")?;
        get_overview(path, contents)
    }
}
impl Display for Overview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted_path = formatted_path(&self.path);
        writeln!(f, "{formatted_path}")?;

        writeln!(f, "Imports:")?;
        if self.uses.0.is_empty() {
            writeln!(f, "  (none)")?;
        } else {
            for import in self.uses.0.iter() {
                writeln!(f, "  {import}")?;
            }
        }

        writeln!(f, "\nStructs:")?;
        if self.structs.is_empty() {
            writeln!(f, "  (none)")?;
        } else {
            for st in self.structs.iter() {
                writeln!(f, "  {st}")?;
            }
        }

        // writeln!(f, "\nFunctions:")?;
        // for function in self.functions.iter() {
        //     writeln!(f, "  {}", function.sig.ident)?;
        // }

        Ok(())
    }
}
impl Diff for Overview {
    type Diff = OverviewDiff;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        let uses_diff = self.uses.diff_with(&other.uses);
        let structs_diff = self.structs.diff_with(&other.structs);
        let file1 = self.path.clone();
        let file2 = other.path.clone();

        OverviewDiff {
            file1,
            file2,
            uses_diff,
            structs_diff,
        }
    }
}

pub struct OverviewDiff {
    file1: PathBuf,
    file2: PathBuf,
    uses_diff: UsesDiff,
    structs_diff: StructsDiff,
}
impl Display for OverviewDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fp1 = &self.file1.to_str().unwrap();
        let fp2 = &self.file2.to_str().unwrap();
        let header = underlined(&format!("{fp1} -> {fp2}"));
        writeln!(f, "{header}")?;

        writeln!(f, "{}", underlined("Use"))?;
        writeln!(f, "{}", self.uses_diff)?;

        writeln!(f, "Structs:")?;
        // if self.structs_diff.added.is_empty() && self.structs_diff.removed.is_empty() {
        //     writeln!(f, "  (none)")?;
        // } else {
        //     for struct_ in self.structs_diff.added.iter() {
        //         writeln!(f, "  + {struct_}")?;
        //     }
        //     for struct_ in self.structs_diff.removed.iter() {
        //         writeln!(f, "  - {struct_}")?;
        //     }
        // }

        Ok(())
    }
}

/// Returns the pathname with an underline of the same length.
///
///  appears as:
///  foo/bar.rs
///  ¯¯¯¯¯¯¯¯¯¯
/// Panics on non-UTF-8 paths.
fn formatted_path(path: &Path) -> String {
    let path = path.to_str().unwrap();
    underlined(path)
}

/// Returns the string with an underline of the same length.
fn underlined(s: &str) -> String {
    let underline = "¯".repeat(s.len());
    format!("{s}\n{underline}")
}
