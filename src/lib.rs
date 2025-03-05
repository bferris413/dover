use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use overview::enums::{Enum, Enums, EnumsDiff};
use overview::traits::{Trait, Traits};
use quote::ToTokens;
use syn::{File, Item, ItemFn};
use syn::{ItemUse, Visibility};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
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
    let mut enums = Vec::new();
    let mut traits = Vec::new();

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
            Item::Enum(item_enum) => {
                enums.push(item_enum);
            }
            Item::Trait(item_trait) => {
                traits.push(item_trait);
            }
            _ => {}
        }
    }

    let traits = traits.into_iter().map(Trait::from).collect();
    let traits = Traits::from(traits);
    dbg!(traits);

    let structs = structs.into_iter().map(Struct::from).collect();
    let structs = Structs::from(structs);

    let enums = enums.into_iter().map(Enum::from).collect();
    let enums = Enums::from(enums);

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
        enums,
    };
    Ok(overview)
}

#[derive(Debug)]
pub struct Overview {
    path: PathBuf,
    uses: Uses,
    structs: Structs,
    enums: Enums,
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
        let enums_diff = self.enums.diff_with(&other.enums);
        let file1 = self.path.clone();
        let file2 = other.path.clone();

        OverviewDiff {
            file1,
            file2,
            uses_diff,
            structs_diff,
            enums_diff,
        }
    }
}

pub struct OverviewDiff {
    file1: PathBuf,
    file2: PathBuf,
    uses_diff: UsesDiff,
    structs_diff: StructsDiff,
    enums_diff: EnumsDiff,
}
impl Display for OverviewDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fp1 = &self.file1.to_str().unwrap();
        let fp2 = &self.file2.to_str().unwrap();
        let header = underlined(&format!("{fp1} -> {fp2}"));
        writeln!(f, "{header}")?;

        writeln!(f, "{}", underlined("Use"))?;
        writeln!(f, "{}", self.uses_diff)?;

        writeln!(f, "{}", underlined("Structs"))?;
        writeln!(f, "{}", self.structs_diff)?;

        writeln!(f, "{}", underlined("Enums"))?;
        writeln!(f, "{}", self.enums_diff)?;

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

/// Formats the left and right columns of a diff as two columns.
///
/// The left column is the original text and the right column is the modified text.
/// Each string in a given list is considered a section, and the first line of
/// each section is always aligned.
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

/// Returns the formatted Rust source of the given items as a string.
fn get_source(items: Vec<Item>) -> String {
    let syn_file = File {
        items,
        shebang: None,
        attrs: vec![],
    };

    prettyplease::unparse(&syn_file)
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

#[derive(Debug, Eq, PartialEq)]
pub struct VisDiff {
    pub old: Vis,
    pub new: Vis,
}
