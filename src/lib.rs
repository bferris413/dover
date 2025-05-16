use std::fmt::{Display, Write};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use colored::Colorize;
use overview::enums::{Enum, Enums, EnumsDiff};
use overview::functions::{Functions, FunctionsDiff};
use overview::impls::{Impls, ImplsDiff};
use overview::traits::{Trait, Traits, TraitsDiff};
use syn::spanned::Spanned;
use syn::{File, Item, ItemFn};
use syn::{ItemUse, Visibility};

use overview::structs::{Struct, Structs, StructsDiff};
use overview::uses::{self, Uses, UsesDiff};

mod git;
mod overview;

pub use git::{get_changed_files, Change as GitChange, ChangedFile, Treeish};

const DEFAULT_MAX_COL_W: usize = 50;

pub trait ByteRange {
    fn old_ranges(&self) -> Vec<Range<usize>>;
    fn new_ranges(&self) -> Vec<Range<usize>>;
}

/// Diff an item with another and return the result.
pub trait Diff {
    type Diff;
    fn diff_with(&self, other: &Self) -> Self::Diff;
}

pub trait View {
    fn as_viewable(&self) -> ViewableDiffs;
}

#[derive(Debug)]
pub struct ViewableDiffs {
    vds: Vec<ViewableDiff>,
}
impl ViewableDiffs {
    pub fn new(diffs: Vec<ViewableDiff>) -> Self {
        ViewableDiffs { vds: diffs }
    }
    pub fn empty() -> ViewableDiffs {
        Self { vds: Vec::new() }
    }
    pub fn append(&mut self, mut diffs: ViewableDiffs) {
        self.vds.append(&mut diffs.vds);
    }
    pub fn appendln(&mut self, mut diffs: ViewableDiffs) {
        self.vds.append(&mut diffs.vds);
        self.vds.push(ViewableDiff { old: Some(vec![(None, Code("\n".to_string()))]), new: Some(vec![(None, Code("\n".to_string()))])});
    }
    pub fn collapse(&mut self) {
        if self.vds.is_empty() {
            return;
        }

        let mut collapsed_old = Vec::new();
        let mut collapsed_new = Vec::new();

        for diff in self.vds.iter_mut() {
            if let Some(ref mut old) = diff.old {
                collapsed_old.append(old);
            }
            if let Some(ref mut new) = diff.new {
                collapsed_new.append(new);
            }
        }

        let mut old = None;
        let mut new = None;

        if !collapsed_old.is_empty() {
            old = Some(collapsed_old);
        }
        if !collapsed_new.is_empty() {
            new = Some(collapsed_new);
        }

        self.vds = vec![ViewableDiff { old, new }];
    }
}

impl Display for ViewableDiffs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // dbg!(self);

        // ---------------------------------------------------------------------------------
        //  None of this is optimized for readability or efficiency. It's barely working.  |
        //                                                                                 |
        //   Edit: A recent bug reminded me how terrible this section is to work in. It    |
        //         needs a complete rewrite.                                               |
        // ---------------------------------------------------------------------------------
        let (mut old_col, mut new_col) = (Vec::new(), Vec::new());

        let old_col_max_width = {
            let mut cur_max = DEFAULT_MAX_COL_W;
            for vd in self.vds.iter() {
                if let Some(ref old) = vd.old {
                    let all_strings: Vec<_> = old.iter().map(|(_, c)| c.0.clone()).collect();
                    let string = all_strings.join("");
                    let local_max = string.lines().map(|l| l.len() + 2).max().unwrap_or(2);
                    cur_max = cur_max.max(local_max);
                }
            }
            cur_max
        };

        for vd in self.vds.iter() {
            let (mut old_section, mut new_section) = (Vec::new(), Vec::new());
            if let Some(old) = &vd.old {
                let mut output_lines = Vec::new();
                let mut line_of_spans = (false, String::new());
                let mut line_of_spans_unformatted_len = 2;

                for (change, code) in old {
                    if code.0.contains("\n") {
                        let mut code_lines = code.0.lines().peekable();
                        let next_span = code_lines.next().unwrap();
                        match change {
                            Some(ExistenceChange::Deleted) => {
                                // println!("writing old del(1) {}", next.red());
                                line_of_spans_unformatted_len += next_span.len();
                                write!(line_of_spans.1, "{}", next_span.red())?;
                                line_of_spans.0 = true;
                            }
                            Some(ExistenceChange::Added) => panic!(),
                            None => {
                                // println!("writing old nil(1) {}", next.normal());
                                line_of_spans_unformatted_len += next_span.len();
                                write!(line_of_spans.1, "{}", next_span.normal())?;
                            }
                        }
                        // println!("pushing old(1) {running_string}");
                        while line_of_spans_unformatted_len < old_col_max_width {
                            line_of_spans.1.push(' ');
                            line_of_spans_unformatted_len += 1;
                        }
                        output_lines.push(format!("{} {}", if line_of_spans.0 { "-".red() } else { " ".red() }, line_of_spans.1.clone()));
                        line_of_spans.1.clear();
                        line_of_spans.0 = false;

                        line_of_spans_unformatted_len = 2;
                        while let Some(line) = code_lines.next() {
                            match change {
                                Some(ExistenceChange::Deleted) => {
                                    if code_lines.peek().is_some()
                                        || (code_lines.peek().is_none() && code.0.ends_with('\n'))
                                    {
                                        // println!("pushing old del(1) {}", line.red());
                                        let gap = old_col_max_width.saturating_sub(line.len());
                                        let full_line_span = format!("{} {line}{}", "-".red(), " ".repeat(gap));
                                        output_lines.push(full_line_span.red().to_string());
                                    } else {
                                        // the last piece and not terminated with \n
                                        // println!("writing old del (2){}", line.red());
                                        line_of_spans_unformatted_len += line.len();
                                        write!(line_of_spans.1, "{}", line.red())?;
                                        line_of_spans.0 = true;
                                    }
                                }
                                Some(ExistenceChange::Added) => panic!(),
                                None => {
                                    if code_lines.peek().is_some()
                                        || (code_lines.peek().is_none() && code.0.ends_with('\n'))
                                    {
                                        // println!("pushing old nil(1) {}", line.normal());
                                        let gap = old_col_max_width.saturating_sub(line.len());
                                        let line = format!("  {line}{}", " ".repeat(gap));
                                        output_lines.push(line.normal().to_string())
                                    } else {
                                        // the last piece and not terminated with \n
                                        // println!("writing old nil (2){}", line.normal());
                                        line_of_spans_unformatted_len += line.len();
                                        write!(line_of_spans.1, "{}", line.normal())?;
                                    }
                                }
                            }
                        }
                    } else {
                        match change {
                            Some(ExistenceChange::Deleted) => {
                                // println!("writing old del(3) {}", code.0.red());
                                line_of_spans_unformatted_len += code.0.len();
                                write!(line_of_spans.1, "{}", code.0.red())?;
                                line_of_spans.0 = true;
                            }
                            Some(ExistenceChange::Added) => panic!(),
                            None => {
                                // println!("writing old nil(3){}", code.0.normal());
                                line_of_spans_unformatted_len += code.0.len();
                                write!(line_of_spans.1, "{}", code.0.normal())?;
                            }
                        }
                    }
                }

                if !line_of_spans.1.is_empty() {
                    while line_of_spans_unformatted_len < old_col_max_width {
                        line_of_spans.1.push(' ');
                        line_of_spans_unformatted_len += 1;
                    }
                    output_lines.push(format!("{} {}", if line_of_spans.0 { "-".red() } else { " ".red() }, line_of_spans.1));
                }

                for line in output_lines.into_iter() {
                    old_section.push(line);
                }
            }

            match &vd.new {
                Some(new) => {
                    let mut output_lines = Vec::new();
                    let mut line_of_spans = (false, String::new());
                    for (change, code) in new {
                        if code.0.contains("\n") {
                            let mut code_lines = code.0.lines().peekable();
                            let next_span = code_lines.next().unwrap();
                            match change {
                                Some(ExistenceChange::Added) => {
                                    // println!("writing new add(1){}", next.green());
                                    write!(line_of_spans.1, "{}", next_span.green())?;
                                    line_of_spans.0 = true;
                                }
                                Some(ExistenceChange::Deleted) => panic!(),
                                None => {
                                    // println!("writing new del(1){}", next.normal());
                                    write!(line_of_spans.1, "{}", next_span.normal())?;
                                }
                            }
                            // println!("pushing new(1) {running_string}");
                            output_lines.push(format!("{} {}", if line_of_spans.0 { "+".green() } else { " ".green() }, line_of_spans.1.clone()));
                            line_of_spans.1.clear();
                            line_of_spans.0 = false; 

                            while let Some(full_line_span) = code_lines.next() {
                                match change {
                                    Some(ExistenceChange::Added) => {
                                        if code_lines.peek().is_some()
                                            || (code_lines.peek().is_none() && code.0.ends_with('\n'))
                                        {
                                            // println!("pushing new add(2){}", line.green());
                                            output_lines.push(format!("{} {}", "+".green(), full_line_span.green().to_string()));
                                        } else {
                                            // the last piece and not terminated with \n
                                            // println!("writing new add(2){}", line.green());
                                            write!(line_of_spans.1, "{}", full_line_span.green())?;
                                            line_of_spans.0 = true;
                                        }
                                    }
                                    Some(ExistenceChange::Deleted) => panic!(),
                                    None => {
                                        if code_lines.peek().is_some()
                                            || (code_lines.peek().is_none() && code.0.ends_with('\n'))
                                        {
                                            // println!("pushing {}", line.normal());
                                            output_lines.push(full_line_span.normal().to_string())
                                        } else {
                                            // the last piece and not terminated with \n
                                            // println!("writing new nil (2){}", line.normal());
                                            write!(line_of_spans.1, "{}", full_line_span.normal())?;
                                        }
                                    }
                                }
                            }
                        } else {
                            match change {
                                Some(ExistenceChange::Added) => {
                                    // println!("writing {}", code.0.green());
                                    write!(line_of_spans.1, "{}", code.0.green())?;
                                    line_of_spans.0 = true;
                                }
                                Some(ExistenceChange::Deleted) => panic!(),
                                None => {
                                    // println!("writing {}", code.0.normal());
                                    write!(line_of_spans.1, "{}", code.0.normal())?;
                                }
                            }
                        }
                    }

                    if !line_of_spans.1.is_empty() {
                        output_lines.push(format!("{} {}", if line_of_spans.0 { "+".green() } else { " ".green() }, line_of_spans.1));
                    }

                    for line in output_lines.into_iter() {
                        new_section.push(line);
                    }
                }
                None => {}
            }
            while old_section.len() < new_section.len() {
                old_section.push(" ".repeat(old_col_max_width));
            }
            while new_section.len() < old_section.len() {
                new_section.push(String::new());
            }

            assert!(!(old_section.is_empty() || new_section.is_empty()));
            assert_eq!(old_section.len(), new_section.len());

            old_section.push(" ".repeat(old_col_max_width));
            new_section.push(String::new());

            // dbg!(&old_section);
            // dbg!(&new_section);

            old_col.append(&mut old_section);
            new_col.append(&mut new_section);
        }

        assert_eq!(old_col.len(), new_col.len());
        let left_right = old_col.iter().zip(new_col.iter());
        let mut formatted_output = String::new();

        for (left, right) in left_right {
            let format_str = format!("{left}      {right}\n");
            formatted_output.push_str(&format_str);
        }

        write!(f, "{}", formatted_output.trim_end())
    }
}

#[derive(Debug)]
pub struct ViewableDiff {
    old: Option<Vec<(Option<ExistenceChange>, Code)>>,
    new: Option<Vec<(Option<ExistenceChange>, Code)>>,
}
impl Display for ViewableDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
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

#[derive(Debug)]
pub struct Code(String);

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

        const MAX_HEADER_WIDTH: usize = 40;

        let fp1 = &self.file1.to_str().unwrap();
        let fp2 = &self.file2.to_str().unwrap();
        let header = underlined(&format!("{fp1} -> {fp2}"));
        let mut string_builder = String::new();
        writeln!(&mut string_builder, "{header}")?;

        if !self.uses_diff.is_empty() {
            let viewable_uses = self.uses_diff.as_viewable();
            writeln!(&mut string_builder, "{}", underlined(&format!("Use{}", " ".repeat(MAX_HEADER_WIDTH - 3))))?;
            writeln!(&mut string_builder, "{viewable_uses}")?;
        }

        if !self.structs_diff.is_empty() {
            let viewable_structs = self.structs_diff.as_viewable();
            writeln!(&mut string_builder, "\n{}", underlined(&format!("Struct{}", " ".repeat(MAX_HEADER_WIDTH - 7))))?;
            writeln!(&mut string_builder, "{viewable_structs}")?;
        }

        if !self.enums_diff.is_empty() {
            let viewable_enums = self.enums_diff.as_viewable();
            writeln!(&mut string_builder, "\n{}", underlined(&format!("Enum{}", " ".repeat(MAX_HEADER_WIDTH - 5))))?;
            writeln!(&mut string_builder, "{viewable_enums}")?;
        }

        if !self.traits_diff.is_empty() {
            let viewable_traits = self.traits_diff.as_viewable();
            writeln!(&mut string_builder, "\n{}", underlined(&format!("Trait{}", " ".repeat(MAX_HEADER_WIDTH - 6))))?;
            writeln!(&mut string_builder, "{viewable_traits}",)?;
        }

        if !self.functions_diff.is_empty() {
            let viewable_funcs = self.functions_diff.as_viewable();
            writeln!(&mut string_builder, "\n{}", underlined(&format!("Function{}", " ".repeat(MAX_HEADER_WIDTH - 9))))?;
            writeln!(&mut string_builder, "{}", viewable_funcs)?;
        }

        if !self.impls_diff.is_empty() {
            let viewable_impls = self.impls_diff.as_viewable();
            writeln!(&mut string_builder, "\n{}", underlined(&format!("Impl{}", " ".repeat(MAX_HEADER_WIDTH - 5))))?;
            writeln!(&mut string_builder, "{viewable_impls}",)?;
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
impl ByteRange for VisDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let old_range = self.old.span().byte_range();
        if old_range.is_empty() {
            Vec::new()
        } else {
            vec![old_range]
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let new_range = self.new.span().byte_range();
        if new_range.is_empty() {
            Vec::new()
        } else {
            vec![new_range]
        }
    }
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

#[macro_export]
macro_rules! collect_src_maps {
    ($($arg:expr),* $(,)?) => {{
        let mut old_src_map = Vec::new();
        let mut new_src_map = Vec::new();
        $(
            if let Some(ref diff) = $arg {
                let mut old_ranges = diff.old_ranges();
                let mut new_ranges = diff.new_ranges();

                old_ranges.retain(|r| ! r.is_empty());
                new_ranges.retain(|r| ! r.is_empty());
                if !old_ranges.is_empty() {
                    old_src_map.append(&mut old_ranges);
                }
                if !new_ranges.is_empty() {
                    new_src_map.append(&mut new_ranges);
                }

            }
        )*
        old_src_map.sort_by(|a, b| a.start.cmp(&b.start));
        old_src_map.sort_by(|a, b| a.end.cmp(&b.end));

        new_src_map.sort_by(|a, b| a.start.cmp(&b.start));
        new_src_map.sort_by(|a, b| a.end.cmp(&b.end));
        (old_src_map, new_src_map)
    }};
}
