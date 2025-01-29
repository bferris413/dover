use std::collections::HashSet;
use std::fmt::Display;
use std::fs;

use anyhow::{Context, Result};
use syn::ItemUse;
use syn::{File, Item, ItemFn, UseTree};

/// Get an overview of a given Rust file.
pub fn get_overview(path: &str) -> Result<Overview> {
    let contents = fs::read_to_string(path).context("Error reading file at {path}")?;
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

    let mut use_paths = HashSet::new();
    for r#use in use_statements.iter() {
        // let visibility = import.vis;
        let tree = &r#use.tree;

        let paths = get_paths_from_usetree(tree);
        use_paths.extend(paths.into_iter());
    }

    let overview = Overview {
        uses: use_paths,
        functions,
    };
    Ok(overview)
}

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

/// A single import, without nesting, groups, or renames.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Use(pub String);
impl Display for Use {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
pub struct Overview {
    uses: HashSet<Use>,
    functions: Vec<ItemFn>,
}
impl Display for Overview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Imports:")?;
        if self.uses.is_empty() {
            writeln!(f, "  (none)")?;
        } else {
            for import in self.uses.iter() {
                writeln!(f, "  {import}")?;
            }
        }

        writeln!(f, "\nFunctions:")?;
        for function in self.functions.iter() {
            writeln!(f, "  {}", function.sig.ident)?;
        }

        Ok(())
    }
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
}
