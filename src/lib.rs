use std::fmt::Display;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use syn::ItemUse;
use syn::{File, Item, ItemFn, UseTree};

mod git;

pub use git::{get_changed_files, ChangeType, ChangedFile};

fn get_overview(path: PathBuf, contents: String) -> Result<Overview> {
    let file: File = syn::parse_file(&contents).context("Error parsing {path}")?;
    let mut use_statements = Vec::new();
    let mut functions = Vec::new();

    for item in file.items {
        match item {
            Item::Use(item_use @ ItemUse { .. }) => {
                use_statements.push(item_use);
            }
            Item::Fn(item_fn @ ItemFn { .. }) => {
                functions.push(item_fn);
            }
            _ => {}
        }
    }

    let mut use_paths = Vec::new();
    for r#use in use_statements.iter() {
        // let visibility = import.vis;
        let tree = &r#use.tree;

        let paths = get_paths_from_usetree(tree);
        use_paths.extend(paths.into_iter());
    }

    let overview = Overview {
        path,
        uses: Uses::from(use_paths),
    };
    Ok(overview)
}

/// Extract and return a collection of single `use` statements from a `UseTree`.
fn get_paths_from_usetree(tree: &UseTree) -> Vec<Use> {
    let mut paths = Vec::new();
    match tree {
        syn::UseTree::Path(path) => {
            let ident = &path.ident;
            let sub_paths = get_paths_from_usetree(&path.tree);
            for sub_path in sub_paths {
                let import = format!("{}::{}", ident, sub_path);
                paths.push(Use(import));
            }
        }
        syn::UseTree::Name(name) => {
            paths.push(Use(name.ident.to_string()));
        }
        syn::UseTree::Rename(rename) => {
            paths.push(Use(rename.ident.to_string()));
        }
        syn::UseTree::Glob(_) => {
            paths.push(Use("*".to_string()));
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                let sub_paths = get_paths_from_usetree(tree);
                paths.extend(sub_paths);
            }
        }
    }
    paths
}

/// A collection of `use` statements.
///
/// The internal representation is sorted and deduped.
#[derive(Debug)]
pub struct Uses(Vec<Use>);
impl Uses {
    /// Creates a complete set of `use` statements from a list of `Use`s.
    pub fn from(mut uses: Vec<Use>) -> Self {
        uses.sort();
        uses.dedup();
        Uses(uses)
    }
}
impl Diff for Uses {
    type Diff = UsesDiff;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        let mut removed = Vec::new();
        let mut added = Vec::new();

        debug_assert!(self.0.is_sorted());
        debug_assert!(other.0.is_sorted());

        for use_ in &self.0 {
            if let Err(_e) = other.0.binary_search(use_) {
                // TODO: switch contents to references
                removed.push(use_.clone());
            }
        }

        for use_ in &other.0 {
            if let Err(_e) = self.0.binary_search(use_) {
                added.push(use_.clone());
            }
        }

        UsesDiff { added, removed }
    }
}
impl Deref for Uses {
    type Target = [Use];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A single use/import statement, without nesting, groups, or renames.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Use(String);
impl Display for Use {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
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
        let fp2 = &self.file2.to_str().unwrap();
        let header = underlined(&format!("{fp1} -> {fp2}"));
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

pub trait Diff {
    type Diff;
    fn diff_with(&self, other: &Self) -> Self::Diff;
}

pub struct UsesDiff {
    added: Vec<Use>,
    removed: Vec<Use>,
}
impl Display for UsesDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{{}}")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_paths_from_usetree_path() {
        let use_tree: syn::UseTree = syn::parse_str("std::fs::File").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(paths, vec![Use("std::fs::File".to_string())]);
    }

    #[test]
    fn test_get_paths_from_usetree_rename() {
        let use_tree: syn::UseTree = syn::parse_str("std::fs::File as StdFile").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(paths, vec![Use("std::fs::File".to_string())]);
    }

    #[test]
    fn test_get_paths_from_usetree_glob() {
        let use_tree: syn::UseTree = syn::parse_str("std::fs::*").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(paths, vec![Use("std::fs::*".to_string())]);
    }

    #[test]
    fn test_get_paths_from_usetree_group() {
        let use_tree: syn::UseTree =
            syn::parse_str("std::{fs, io::{self, empty}, error::Error}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(
            paths,
            vec![
                Use("std::fs".to_string()),
                Use("std::io::self".to_string()),
                Use("std::io::empty".to_string()),
                Use("std::error::Error".to_string())
            ]
        );
    }

    #[test]
    fn test_get_paths_from_usetree_deeply_nested() {
        let use_tree: syn::UseTree = syn::parse_str("a::b::c::d::e").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(paths, vec![Use("a::b::c::d::e".to_string())]);
    }

    #[test]
    fn test_get_paths_from_usetree_multiple_levels_of_groups() {
        let use_tree: syn::UseTree = syn::parse_str("a::{b::{c, d}, e}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(
            paths,
            vec![
                Use("a::b::c".to_string()),
                Use("a::b::d".to_string()),
                Use("a::e".to_string())
            ]
        );
    }

    #[test]
    fn test_get_paths_from_usetree_self_in_nested_groups() {
        let use_tree: syn::UseTree = syn::parse_str("a::{self, b::{self, c}}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(
            paths,
            vec![
                Use("a::self".to_string()),
                Use("a::b::self".to_string()),
                Use("a::b::c".to_string())
            ]
        );
    }

    #[test]
    fn test_get_paths_from_usetree_aliased_paths_in_groups() {
        let use_tree: syn::UseTree = syn::parse_str("a::{b as x, c::{d as y, e}}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(
            paths,
            vec![
                Use("a::b".to_string()),
                Use("a::c::d".to_string()),
                Use("a::c::e".to_string())
            ]
        );
    }

    #[test]
    fn test_get_paths_from_usetree_glob_with_other_paths() {
        let use_tree: syn::UseTree = syn::parse_str("a::{*, b}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(
            paths,
            vec![Use("a::*".to_string()), Use("a::b".to_string())]
        );
    }

    #[test]
    fn test_get_paths_from_usetree_empty_groups() {
        let use_tree: syn::UseTree = syn::parse_str("a::{}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_get_paths_from_usetree_trailing_commas_in_groups() {
        let use_tree: syn::UseTree = syn::parse_str("a::{b, c,}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(
            paths,
            vec![Use("a::b".to_string()), Use("a::c".to_string())]
        );
    }

    #[test]
    fn test_get_paths_from_usetree_complex_combinations() {
        let use_tree: syn::UseTree = syn::parse_str("a::{b::{c, d::e}, f::{g, h::i}}").unwrap();
        let paths = get_paths_from_usetree(&use_tree);
        assert_eq!(
            paths,
            vec![
                Use("a::b::c".to_string()),
                Use("a::b::d::e".to_string()),
                Use("a::f::g".to_string()),
                Use("a::f::h::i".to_string())
            ]
        );
    }

    #[test]
    fn test_diff_with_no_changes() {
        let uses1 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
        ]);
        let uses2 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
        ]);

        let diff = uses1.diff_with(&uses2);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_with_added_imports() {
        let uses1 = Uses::from(vec![Use("std::fs::File".to_string())]);
        let uses2 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
        ]);

        let diff = uses1.diff_with(&uses2);
        assert_eq!(diff.added, vec![Use("std::io::Read".to_string())]);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_with_removed_imports() {
        let uses1 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
        ]);
        let uses2 = Uses::from(vec![Use("std::fs::File".to_string())]);

        let diff = uses1.diff_with(&uses2);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec![Use("std::io::Read".to_string())]);
    }

    #[test]
    fn test_diff_with_added_and_removed_imports() {
        let uses1 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
        ]);
        let uses2 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Write".to_string()),
        ]);

        let diff = uses1.diff_with(&uses2);
        assert_eq!(diff.added, vec![Use("std::io::Write".to_string())]);
        assert_eq!(diff.removed, vec![Use("std::io::Read".to_string())]);
    }

    #[test]
    fn test_diff_with_multiple_changes() {
        let uses1 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
            Use("std::path::Path".to_string()),
        ]);
        let uses2 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::fs::OpenOptions".to_string()),
            Use("std::io::Write".to_string()),
        ]);

        let diff = uses1.diff_with(&uses2);
        assert_eq!(
            diff.added,
            vec![
                Use("std::fs::OpenOptions".to_string()),
                Use("std::io::Write".to_string()),
            ]
        );
        assert_eq!(
            diff.removed,
            vec![
                Use("std::io::Read".to_string()),
                Use("std::path::Path".to_string())
            ]
        );
    }
}
