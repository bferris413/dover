use std::fmt::{Display, Write};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use overview::enums::{Enum, Enums, EnumsDiff};
use overview::functions::{Functions, FunctionsDiff};
use overview::impls::{Impls, ImplsDiff};
use overview::traits::{Trait, Traits, TraitsDiff};
use syn::{File, Item, ItemFn};
use syn::{ItemUse, Visibility};

use overview::structs::{Struct, Structs, StructsDiff};
use overview::uses::{self, Uses, UsesDiff};

mod git;
mod overview;

pub use git::{get_changed_files, Change as GitChange, ChangedFile, Treeish};

/// Diff an item with another and return the result.
pub trait Diff {
    type Diff;
    fn diff_with(&self, other: &Self) -> Self::Diff;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum Change {
    #[default]
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

fn get_overview(path: PathBuf, source: String) -> Result<Overview> {
    let file: File = syn::parse_file(&source).context("Error parsing {path}")?;
    let source = SourceFile::from(source);
    let mut use_statements = Vec::new();
    let mut functions = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut traits = Vec::new();
    let mut impls = Vec::new();

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
            Item::Impl(item_impl) => {
                impls.push(item_impl);
            }
            _ => {}
        }
    }

    let traits = traits
        .into_iter()
        .map(|t| Trait::new(t, source.clone()))
        .collect();
    let traits = Traits::from(traits);

    let structs = structs
        .into_iter()
        .map(|s| Struct::new(s, source.clone()))
        .collect();
    let structs = Structs::from(structs);

    let enums = enums
        .into_iter()
        .map(|e| Enum::new(e, source.clone()))
        .collect();
    let enums = Enums::from(enums);

    let functions = Functions::new_freestanding(functions, source.clone());
    let impls = Impls::new(impls, source);

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
        traits,
        functions,
        impls,
    };
    Ok(overview)
}

#[derive(Debug)]
pub struct Overview {
    path: PathBuf,
    uses: Uses,
    structs: Structs,
    enums: Enums,
    traits: Traits,
    functions: Functions,
    impls: Impls,
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

        if !self.uses.0.is_empty() {
            writeln!(f, "Imports:")?;
            for import in self.uses.0.iter() {
                writeln!(f, "{import}")?;
            }
        }

        if !self.structs.is_empty() {
            writeln!(f, "\nStructs:")?;
            for st in self.structs.iter() {
                writeln!(f, "{st}")?;
            }
        }

        if !self.enums.is_empty() {
            writeln!(f, "\nEnums:")?;
            for en in self.enums.iter() {
                writeln!(f, "{en}")?;
            }
        }

        if !self.traits.is_empty() {
            writeln!(f, "\nTraits:")?;
            for tr in self.traits.iter() {
                writeln!(f, "{tr}")?;
            }
        }

        if !self.functions.is_empty() {
            writeln!(f, "\nFunctions:")?;
            for func in self.functions.functions().iter() {
                writeln!(f, "{func}")?;
            }
        }

        if !self.impls.is_empty() {
            for imp in self.impls.impls().iter() {
                writeln!(f, "{imp}")?;
            }
        }

        Ok(())
    }
}
impl Diff for Overview {
    type Diff = OverviewDiff;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        let uses_diff = self.uses.diff_with(&other.uses);
        let structs_diff = self.structs.diff_with(&other.structs);
        let enums_diff = self.enums.diff_with(&other.enums);
        let traits_diff = self.traits.diff_with(&other.traits);
        let functions_diff = self.functions.diff_with(&other.functions);
        let impls_diff = self.impls.diff_with(&other.impls);
        let file1 = self.path.clone();
        let file2 = other.path.clone();

        OverviewDiff {
            file1,
            file2,
            uses_diff,
            structs_diff,
            enums_diff,
            traits_diff,
            functions_diff,
            impls_diff,
        }
    }
}

pub struct OverviewDiff {
    file1: PathBuf,
    file2: PathBuf,
    uses_diff: UsesDiff,
    structs_diff: StructsDiff,
    enums_diff: EnumsDiff,
    traits_diff: TraitsDiff,
    functions_diff: FunctionsDiff,
    impls_diff: ImplsDiff,
}
impl OverviewDiff {
    pub fn all_empty(&self) -> bool {
        self.uses_diff.is_empty()
            && self.structs_diff.is_empty()
            && self.enums_diff.is_empty()
            && self.traits_diff.is_empty()
            && self.functions_diff.is_empty()
            && self.impls_diff.is_empty()
    }
}
impl Display for OverviewDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.all_empty() {
            return Ok(());
        }

        let fp1 = &self.file1.to_str().unwrap();
        let fp2 = &self.file2.to_str().unwrap();
        let header = underlined(&format!("{fp1} -> {fp2}"));
        let mut string_builder = String::new();
        writeln!(&mut string_builder, "{header}")?;

        if !self.uses_diff.is_empty() {
            writeln!(&mut string_builder, "{}", underlined("Use"))?;
            writeln!(&mut string_builder, "{}", self.uses_diff)?;
        }

        if !self.structs_diff.is_empty() {
            writeln!(&mut string_builder, "{}", underlined("Structs"))?;
            writeln!(&mut string_builder, "{}", self.structs_diff)?;
        }

        if !self.enums_diff.is_empty() {
            writeln!(&mut string_builder, "{}", underlined("Enums"))?;
            writeln!(&mut string_builder, "{}", self.enums_diff)?;
        }

        if !self.traits_diff.is_empty() {
            writeln!(&mut string_builder, "{}", underlined("Traits"))?;
            writeln!(&mut string_builder, "{}", self.traits_diff)?;
        }

        if !self.functions_diff.is_empty() {
            writeln!(&mut string_builder, "{}", underlined("Functions"))?;
            writeln!(&mut string_builder, "{}", self.functions_diff)?;
        }

        if !self.impls_diff.is_empty() {
            writeln!(&mut string_builder, "{}", underlined("Impls"))?;
            writeln!(&mut string_builder, "{}", self.impls_diff)?;
        }

        while string_builder.ends_with('\n') {
            string_builder.pop().unwrap();
        }

        write!(f, "{string_builder}")
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

    let mut should_pop_newline = false;
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
            should_pop_newline = true;
        }
    }

    if should_pop_newline {
        formatted_output.pop(); // removed extra newline
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
impl Diff for Visibility {
    type Diff = Option<VisDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        Some(VisDiff {
            old: self.clone(),
            new: other.clone(),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct VisDiff {
    pub old: Visibility,
    pub new: Visibility,
}

/// Cheaply cloneable reference to the original source.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SourceFile(Arc<String>);
impl From<String> for SourceFile {
    fn from(value: String) -> Self {
        let source = Arc::new(value);
        SourceFile(source)
    }
}
