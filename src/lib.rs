use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use quote::ToTokens;
use syn::ItemUse;
use syn::{File, Item, ItemFn};

use overview::uses::{self, Uses, UsesDiff};

mod git;
mod overview;

pub use git::{get_changed_files, ChangeType, ChangedFile};

/// Diff an item with another and return the result.
pub trait Diff {
    type Diff;
    fn diff_with(&self, other: &Self) -> Self::Diff;
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

    dbg!(&structs);
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
    };
    Ok(overview)
}

#[derive(Debug)]
pub struct Overview {
    path: PathBuf,
    uses: Uses,
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
        let file1 = self.path.clone();
        let file2 = other.path.clone();

        OverviewDiff {
            file1,
            file2,
            uses_diff,
        }
    }
}

pub struct OverviewDiff {
    file1: PathBuf,
    file2: PathBuf,
    uses_diff: UsesDiff,
}
impl Display for OverviewDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fp1 = &self.file1.to_str().unwrap();
        let header = underlined(&format!("{fp1}"));
        writeln!(f, "{header}")?;

        writeln!(f, "Imports:")?;
        if self.uses_diff.added.is_empty() && self.uses_diff.removed.is_empty() {
            writeln!(f, "  (none)")?;
        } else {
            for import in self.uses_diff.added.iter() {
                writeln!(f, "  + {import}")?;
            }
            for import in self.uses_diff.removed.iter() {
                writeln!(f, "  - {import}")?;
            }
        }

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
