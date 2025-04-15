use crate::{Diff, ExistenceChange};
use colored::Colorize;
use std::{
    fmt::{Display, Write},
    ops::Deref,
};
use syn::UseTree;

/// A collection of `use` statements.
///
/// The internal representation is sorted and deduped.
#[derive(Debug)]
pub struct Uses(pub Vec<Use>);
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
        let mut diffs = Vec::new();

        debug_assert!(self.0.is_sorted());
        debug_assert!(other.0.is_sorted());

        for use_ in &self.0 {
            if let Err(_e) = other.0.binary_search(use_) {
                diffs.push(UseDiff {
                    change: ExistenceChange::Deleted,
                    use_: use_.clone(),
                });
            }
        }

        for use_ in &other.0 {
            if let Err(_e) = self.0.binary_search(use_) {
                diffs.push(UseDiff {
                    change: ExistenceChange::Added,
                    use_: use_.clone(),
                });
            }
        }

        diffs.sort_by(|d1, d2| d1.use_.0.cmp(&d2.use_.0));

        UsesDiff { diffs }
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

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct UseDiff {
    change: ExistenceChange,
    use_: Use,
}
impl Display for UseDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.change, self.use_)
    }
}

pub struct UsesDiff {
    diffs: Vec<UseDiff>,
}
impl UsesDiff {
    pub(crate) fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }
}
impl Display for UsesDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.diffs.is_empty() {
            return writeln!(f, "(no changes)");
        }

        let (mut left_col, mut right_col) = (String::new(), String::new());
        let mut any_ex_diffs = false;
        for diff in self.diffs.iter() {
            any_ex_diffs = true;
            match diff.change {
                ExistenceChange::Added => {
                    write!(right_col, "{diff}")?;
                }
                ExistenceChange::Deleted => {
                    write!(left_col, "{diff}")?;
                }
            }
        }
        if any_ex_diffs {
            let output = crate::format_as_columns(&vec![left_col], &vec![right_col]);
            writeln!(f, "{output}")?;
        }

        Ok(())
    }
}

/// Extract and return a collection of single `use` statements from a `UseTree`.
///
/// The collection is not guaranteed to be sorted or deduped.
pub fn get_paths_from_usetree(tree: &UseTree) -> Vec<Use> {
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
        assert!(diff.diffs.is_empty());
    }

    #[test]
    fn test_diff_with_added_imports() {
        let uses1 = Uses::from(vec![Use("std::fs::File".to_string())]);
        let uses2 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
        ]);

        let diff = uses1.diff_with(&uses2);
        let exp_diff = UseDiff {
            change: (ExistenceChange::Added),
            use_: Use("std::io::Read".to_string()),
        };
        assert_eq!(diff.diffs, vec![exp_diff]);
    }

    #[test]
    fn test_diff_with_removed_imports() {
        let uses1 = Uses::from(vec![
            Use("std::fs::File".to_string()),
            Use("std::io::Read".to_string()),
        ]);
        let uses2 = Uses::from(vec![Use("std::fs::File".to_string())]);

        let diff = uses1.diff_with(&uses2);
        let exp_diff = vec![UseDiff {
            change: (ExistenceChange::Deleted),
            use_: Use("std::io::Read".to_string()),
        }];
        assert_eq!(diff.diffs, exp_diff);
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
        let exp_diff = vec![
            UseDiff {
                change: (ExistenceChange::Deleted),
                use_: Use("std::io::Read".to_string()),
            },
            UseDiff {
                change: (ExistenceChange::Added),
                use_: Use("std::io::Write".to_string()),
            },
        ];
        assert_eq!(diff.diffs, exp_diff);
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
        let exp_diff = vec![
            UseDiff {
                change: (ExistenceChange::Added),
                use_: Use("std::fs::OpenOptions".to_string()),
            },
            UseDiff {
                change: (ExistenceChange::Deleted),
                use_: Use("std::io::Read".to_string()),
            },
            UseDiff {
                change: (ExistenceChange::Added),
                use_: Use("std::io::Write".to_string()),
            },
            UseDiff {
                change: (ExistenceChange::Deleted),
                use_: Use("std::path::Path".to_string()),
            },
        ];
        assert_eq!(diff.diffs, exp_diff);
    }
}
