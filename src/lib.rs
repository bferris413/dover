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

mod config;
mod git;
mod html;
mod overview;

pub use config::Config;
pub use git::{Change as GitChange, ChangedFile, Treeish, get_changed_files};
pub use html::{HTML_BOILERPLATE, HTML_EPILOGUE};

const ASCII_LINE_FEED: u8 = 10;
const TERMINAL_HEADER_WIDTH: usize = 80;

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

pub trait Html {
    fn to_html(&self) -> String;
}

#[derive(Debug)]
pub struct ViewableDiffs {
    vds: Vec<ViewableDiff>,
}
impl ViewableDiffs {
    pub fn new(diffs: Vec<ViewableDiff>) -> Self {
        ViewableDiffs { vds: diffs }
    }

    pub fn is_empty(&self) -> bool {
        self.vds.is_empty()
    }

    pub fn empty() -> ViewableDiffs {
        Self { vds: Vec::new() }
    }

    pub fn append(&mut self, mut diffs: ViewableDiffs) {
        self.vds.append(&mut diffs.vds);
    }

    pub fn appendln(&mut self, mut diffs: ViewableDiffs) {
        self.vds.append(&mut diffs.vds);
        self.vds.push(ViewableDiff::newline());
    }

    pub fn collapse(&mut self) {
        if self.vds.is_empty() {
            return;
        }

        let mut old_items = Vec::new();
        let mut new_items = Vec::new();
        for diff in &mut self.vds {
            if let Some(old) = diff.old.take() {
                old_items.push(old);
            }
            if let Some(new) = diff.new.take() {
                new_items.push(new);
            }
        }
        let mut old_items = old_items.into_iter();
        let mut new_items = new_items.into_iter();
        let mut aligned = Vec::new();

        loop {
            let old = old_items.next();
            let new = new_items.next();
            if old.is_none() && new.is_none() {
                break;
            }
            aligned.push(ViewableDiff { old, new });
        }

        self.vds = aligned;
    }

    fn to_terminal(&self) -> String {
        self.vds
            .iter()
            .filter_map(ViewableDiff::to_terminal)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl ViewableDiff {
    fn to_terminal(&self) -> Option<String> {
        let old_lines = self
            .old
            .as_ref()
            .map(|old| terminal_lines(old, ExistenceChange::Deleted))
            .unwrap_or_default();
        let new_lines = self
            .new
            .as_ref()
            .map(|new| terminal_lines(new, ExistenceChange::Added))
            .unwrap_or_default();
        let lines = merge_terminal_sides(old_lines, new_lines);

        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}

#[derive(Debug)]
struct TerminalLine {
    spans: Vec<(bool, String)>,
    change: Option<ExistenceChange>,
}

impl TerminalLine {
    fn plain(&self) -> String {
        self.spans.iter().map(|(_, text)| text.as_str()).collect()
    }

    fn render(&self) -> String {
        self.render_with_change(self.change)
    }

    fn render_with_change(&self, change: Option<ExistenceChange>) -> String {
        let mut rendered = String::new();
        for (changed, text) in &self.spans {
            push_terminal_fragment(
                &mut rendered,
                text,
                *changed || (self.change.is_none() && change.is_some()),
                change.unwrap_or(ExistenceChange::Added),
            );
        }

        let marker = match change {
            Some(ExistenceChange::Deleted) => "-".red().to_string(),
            Some(ExistenceChange::Added) => "+".green().to_string(),
            None => " ".to_owned(),
        };
        format!("{marker} {rendered}")
    }

    fn unchanged(text: String) -> Self {
        Self {
            spans: vec![(false, text)],
            change: None,
        }
    }

    fn slice(&self, start: usize, end: usize, prefix: &str) -> Self {
        let mut spans = Vec::new();
        if !prefix.is_empty() {
            spans.push((false, prefix.to_owned()));
        }

        let mut offset = 0;
        for (changed, text) in &self.spans {
            let span_start = offset;
            let span_end = offset + text.len();
            let overlap_start = start.max(span_start);
            let overlap_end = end.min(span_end);
            if overlap_start < overlap_end {
                spans.push((
                    *changed,
                    text[overlap_start - span_start..overlap_end - span_start].to_owned(),
                ));
            }
            offset = span_end;
        }

        Self {
            spans,
            change: self.change,
        }
    }
}

fn terminal_lines(
    spans: &[(Option<ExistenceChange>, Code)],
    expected_change: ExistenceChange,
) -> Vec<TerminalLine> {
    let mut rendered_lines = Vec::new();
    let mut current_line = Vec::new();
    let mut line_has_change = false;

    let finish_line =
        |line: &mut Vec<(bool, String)>, has_change: &mut bool, output: &mut Vec<TerminalLine>| {
            if !line.is_empty() {
                output.push(TerminalLine {
                    spans: std::mem::take(line),
                    change: has_change.then_some(expected_change),
                });
            }
            *has_change = false;
        };

    for (change, code) in spans {
        let changed = *change == Some(expected_change);
        let mut remaining = code.0.as_str();

        while let Some(newline) = remaining.find('\n') {
            let fragment = &remaining[..newline];
            if !fragment.is_empty() {
                current_line.push((changed, fragment.to_owned()));
            }
            line_has_change |= changed && !fragment.is_empty();
            finish_line(&mut current_line, &mut line_has_change, &mut rendered_lines);
            remaining = &remaining[newline + 1..];
        }

        if !remaining.is_empty() {
            current_line.push((changed, remaining.to_owned()));
            line_has_change |= changed;
        }
    }

    if !current_line.is_empty() {
        finish_line(&mut current_line, &mut line_has_change, &mut rendered_lines);
    }

    rendered_lines
}

fn merge_terminal_sides(old: Vec<TerminalLine>, new: Vec<TerminalLine>) -> Vec<String> {
    if old.is_empty() || new.is_empty() {
        return old
            .into_iter()
            .chain(new)
            .map(|line| line.render())
            .collect();
    }

    let matches = unchanged_line_matches(&old, &new);
    let mut result = Vec::new();
    let (mut old_start, mut new_start) = (0, 0);
    for (old_match, new_match) in matches {
        render_changed_block(
            &mut result,
            &old[old_start..old_match],
            &new[new_start..new_match],
        );
        result.push(old[old_match].render());
        old_start = old_match + 1;
        new_start = new_match + 1;
    }
    render_changed_block(&mut result, &old[old_start..], &new[new_start..]);
    result
}

fn unchanged_line_matches(old: &[TerminalLine], new: &[TerminalLine]) -> Vec<(usize, usize)> {
    let mut lengths = vec![vec![0; new.len() + 1]; old.len() + 1];
    for old_index in (0..old.len()).rev() {
        for new_index in (0..new.len()).rev() {
            let is_match = old[old_index].change.is_none()
                && new[new_index].change.is_none()
                && old[old_index].plain() == new[new_index].plain();
            lengths[old_index][new_index] = if is_match {
                lengths[old_index + 1][new_index + 1] + 1
            } else {
                lengths[old_index + 1][new_index].max(lengths[old_index][new_index + 1])
            };
        }
    }

    let mut matches = Vec::new();
    let (mut old_index, mut new_index) = (0, 0);
    while old_index < old.len() && new_index < new.len() {
        let is_match = old[old_index].change.is_none()
            && new[new_index].change.is_none()
            && old[old_index].plain() == new[new_index].plain();
        if is_match {
            matches.push((old_index, new_index));
            old_index += 1;
            new_index += 1;
        } else if lengths[old_index + 1][new_index] >= lengths[old_index][new_index + 1] {
            old_index += 1;
        } else {
            new_index += 1;
        }
    }
    matches
}

fn render_changed_block(result: &mut Vec<String>, old: &[TerminalLine], new: &[TerminalLine]) {
    if let ([old_line], [new_line]) = (old, new)
        && let Some(factored) = factor_changed_line(old_line, new_line)
    {
        result.extend(factored.into_iter().map(|line| line.render()));
    } else {
        result.extend(
            old.iter()
                .map(|line| line.render_with_change(Some(ExistenceChange::Deleted))),
        );
        result.extend(
            new.iter()
                .map(|line| line.render_with_change(Some(ExistenceChange::Added))),
        );
    }
}

fn factor_changed_line(old: &TerminalLine, new: &TerminalLine) -> Option<Vec<TerminalLine>> {
    if old.change != Some(ExistenceChange::Deleted) || new.change != Some(ExistenceChange::Added) {
        return None;
    }

    let old_plain = old.plain();
    let new_plain = new.plain();
    let old_leading = leading_unchanged(old);
    let new_leading = leading_unchanged(new);
    let prefix_len = common_prefix_len(&old_leading, &new_leading);
    let old_trailing = trailing_unchanged(old);
    let new_trailing = trailing_unchanged(new);
    let suffix_len = common_suffix_len(&old_trailing, &new_trailing)
        .min(old_plain.len() - prefix_len)
        .min(new_plain.len() - prefix_len);

    let prefix = &old_plain[..prefix_len];
    let suffix = &old_plain[old_plain.len() - suffix_len..];
    if !prefix.trim_end().ends_with('(') || !suffix.trim_start().starts_with(')') {
        return None;
    }

    let indentation: String = prefix.chars().take_while(|c| c.is_whitespace()).collect();
    let nested_indent = format!("{indentation}    ");
    Some(vec![
        TerminalLine::unchanged(prefix.to_owned()),
        old.slice(prefix_len, old_plain.len() - suffix_len, &nested_indent),
        new.slice(prefix_len, new_plain.len() - suffix_len, &nested_indent),
        TerminalLine::unchanged(format!("{indentation}{}", suffix.trim_start())),
    ])
}

fn leading_unchanged(line: &TerminalLine) -> String {
    line.spans
        .iter()
        .take_while(|(changed, _)| !changed)
        .map(|(_, text)| text.as_str())
        .collect()
}

fn trailing_unchanged(line: &TerminalLine) -> String {
    let trailing: Vec<&str> = line
        .spans
        .iter()
        .rev()
        .take_while(|(changed, _)| !changed)
        .map(|(_, text)| text.as_str())
        .collect();
    trailing.into_iter().rev().collect()
}

fn common_prefix_len(left: &str, right: &str) -> usize {
    left.char_indices()
        .zip(right.chars())
        .take_while(|((_, left), right)| left == right)
        .map(|((index, ch), _)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn common_suffix_len(left: &str, right: &str) -> usize {
    left.char_indices()
        .rev()
        .zip(right.chars().rev())
        .take_while(|((_, left), right)| left == right)
        .map(|((index, _), _)| left.len() - index)
        .last()
        .unwrap_or(0)
}

fn push_terminal_fragment(
    line: &mut String,
    fragment: &str,
    changed: bool,
    expected_change: ExistenceChange,
) {
    if changed {
        match expected_change {
            ExistenceChange::Deleted => write!(line, "{}", fragment.red()).unwrap(),
            ExistenceChange::Added => write!(line, "{}", fragment.green()).unwrap(),
        }
    } else {
        line.push_str(fragment);
    }
}
impl Html for ViewableDiffs {
    fn to_html(&self) -> String {
        let mut html = String::new();

        for vd in self.vds.iter() {
            html.push_str("<tr class=\"diff-item\">");

            let mut deleted_content = String::new();
            let mut has_deleted_content = false;
            if let Some(ref old) = vd.old {
                for diff in old.iter() {
                    let class = match diff.0 {
                        Some(ExistenceChange::Deleted) => {
                            has_deleted_content = true;
                            "deleted"
                        }
                        Some(ExistenceChange::Added) => unreachable!(),
                        None => "",
                    };
                    deleted_content.push_str(&format!(
                        "<span class=\"{}\">{}</span>",
                        class,
                        escape_html(&diff.1.to_string())
                    ));
                }
            }
            if !has_deleted_content {
                html.push_str("<td class=\"empty-content\">");
                html.push_str("</td>");
            } else {
                html.push_str("<td class=\"diff-cell deleted-cell\">");
                html.push_str("<div class=\"diff-cell-scroll\"><pre><code>");
                html.push_str(&deleted_content);
                html.push_str("</code></pre></div>");
                html.push_str("</td>");
            }

            let mut added_content = String::new();
            let mut has_added_content = false;

            if let Some(ref new) = vd.new {
                for diff in new.iter() {
                    let class = match diff.0 {
                        Some(ExistenceChange::Deleted) => unreachable!(),
                        Some(ExistenceChange::Added) => {
                            has_added_content = true;
                            "added"
                        }
                        None => "",
                    };
                    added_content.push_str(&format!(
                        "<span class=\"{}\">{}</span>",
                        class,
                        escape_html(&diff.1.to_string())
                    ));
                }
            }
            if !has_added_content {
                html.push_str("<td class=\"empty-content\">");
                html.push_str("</td>");
            } else {
                html.push_str("<td class=\"diff-cell added-cell\">");
                html.push_str("<div class=\"diff-cell-scroll\"><pre><code>");
                html.push_str(&added_content);
                html.push_str("</code></pre></div>");
                html.push_str("</td>");
            }

            html.push_str("</tr>");
        }

        // let html_with_br = html.replace("\n", "<br>");

        // html_with_br
        html
    }
}

impl Display for ViewableDiffs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_terminal())
    }
}
#[derive(Debug)]
pub struct ViewableDiff {
    old: Option<Vec<(Option<ExistenceChange>, Code)>>,
    new: Option<Vec<(Option<ExistenceChange>, Code)>>,
}
impl ViewableDiff {
    fn newline() -> ViewableDiff {
        ViewableDiff {
            old: Some(vec![(None, Code("\n".to_string()))]),
            new: Some(vec![(None, Code("\n".to_string()))]),
        }
    }
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
impl Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
        use_paths.extend(paths);
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

    pub fn has_visible_changes(&self, config: &Config) -> bool {
        (config.uses.show && !self.uses_diff.is_empty())
            || (config.structs.show && !self.structs_diff.is_empty())
            || (config.enums.show && !self.enums_diff.is_empty())
            || (config.traits.show && !self.traits_diff.is_empty())
            || (config.functions.show && !self.functions_diff.is_empty())
            || (config.impls.show && !self.impls_diff.is_empty())
    }

    pub fn to_html_with_config(&self, config: &Config) -> String {
        if !self.has_visible_changes(config) {
            return String::new();
        }

        let fp1 = self.file1.to_string_lossy();
        let fp2 = self.file2.to_string_lossy();
        let label = if fp1 == fp2 {
            fp1.to_string()
        } else {
            format!("{fp1} → {fp2}")
        };
        let mut html = format!(
            "<details class=\"file-diff\" data-file-path=\"{}\" open><summary>{}</summary>",
            escape_html(&fp1),
            escape_html(&label)
        );

        fn render_section(html: &mut String, title: &str, viewable: &ViewableDiffs) {
            if viewable.is_empty() {
                return;
            }
            html.push_str(&format!(
                "<details class=\"diff-section\" open><summary>{}</summary>",
                escape_html(title)
            ));
            html.push_str("<div class=\"diff-table-wrap\"><table class=\"diff-table\"><colgroup><col><col></colgroup><tbody>");
            html.push_str(&viewable.to_html());
            html.push_str("</tbody></table></div></details>");
        }

        if config.uses.show {
            render_section(&mut html, "Uses", &self.uses_diff.as_viewable());
        }
        if config.structs.show {
            render_section(&mut html, "Structs", &self.structs_diff.as_viewable());
        }
        if config.enums.show {
            render_section(&mut html, "Enums", &self.enums_diff.as_viewable());
        }
        if config.traits.show {
            render_section(&mut html, "Traits", &self.traits_diff.as_viewable());
        }
        if config.functions.show {
            render_section(&mut html, "Functions", &self.functions_diff.as_viewable());
        }
        if config.impls.show {
            render_section(&mut html, "Impls", &self.impls_diff.as_viewable());
        }

        html.push_str("</details>");
        html
    }

    pub fn to_terminal_with_config(&self, config: &Config) -> String {
        if !self.has_visible_changes(config) {
            return String::new();
        }

        let fp1 = &self.file1.to_str().unwrap();
        let fp2 = &self.file2.to_str().unwrap();
        let header = if fp1 == fp2 {
            (*fp1).to_owned()
        } else {
            format!("{fp1} → {fp2}")
        };
        let mut sections = Vec::new();

        macro_rules! render_section {
            ($show:expr, $title:literal, $diff:expr) => {
                if $show && !$diff.is_empty() {
                    let view = $diff.as_viewable();
                    sections.push(format!(
                        "{}\n{}",
                        terminal_header($title),
                        view.to_terminal()
                    ));
                }
            };
        }

        render_section!(config.uses.show, "Uses", self.uses_diff);
        render_section!(config.structs.show, "Structs", self.structs_diff);
        render_section!(config.enums.show, "Enums", self.enums_diff);
        render_section!(config.traits.show, "Traits", self.traits_diff);
        render_section!(config.functions.show, "Functions", self.functions_diff);
        render_section!(config.impls.show, "Impls", self.impls_diff);

        format!("{}\n{}", header.bold().reversed(), sections.join("\n\n"))
    }
}
impl Html for OverviewDiff {
    fn to_html(&self) -> String {
        self.to_html_with_config(&Config::default())
    }
}

impl Display for OverviewDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_terminal_with_config(&Config::default()))
    }
}

fn terminal_header(title: &str) -> String {
    let title: String = title.chars().take(TERMINAL_HEADER_WIDTH).collect();
    format!("{title:<TERMINAL_HEADER_WIDTH$}")
        .bold()
        .reversed()
        .to_string()
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

fn collect_diff_changes(
    source_code: &[u8],
    source_map: &[Range<usize>],
    decl_start: usize,
    sig_end: usize,
    ex: ExistenceChange,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let mut i = decl_start;
    let mut src_i = 0;
    let mut diff_changes = Vec::new();

    while i < sig_end {
        let maybe_diff_index = source_map[src_i..].iter().position(|r| r.contains(&i));
        match maybe_diff_index {
            Some(diff_index) => {
                let diff_range = &source_map[src_i..][diff_index];

                // doesn't make sense that we wouldn't be aligned with the start of a range
                assert_eq!(i, diff_range.start);
                let substring = source_code[i..diff_range.end].to_vec();
                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                diff_changes.push((Some(ex), code));

                src_i = diff_index + 1;
                i = diff_range.end;
            }
            None => {
                let start = i;
                while i < sig_end {
                    let maybe_diff_index = source_map[src_i..].iter().position(|r| r.contains(&i));
                    if maybe_diff_index.is_some() {
                        break;
                    } else {
                        i += 1
                    }
                }
                // We're either off the end or we've found a new diff. Either way,
                // start..i contains our next range
                let substring = source_code[start..i].to_vec();
                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                diff_changes.push((None, code));
            }
        }
    }

    diff_changes
}

// Returns a formatted string to indicate that the full struct isn't being displayed.
fn collect_elided_whitespace(sig_end: usize, source_code: &[u8], item_range_end: usize) -> String {
    let mut fields_start = sig_end;
    while source_code[fields_start].is_ascii_whitespace() && fields_start < item_range_end {
        fields_start += 1;
    }

    let whitespace = String::from_utf8_lossy(&source_code[sig_end..fields_start]);
    format!("{whitespace}..")
}

fn collect_preceding_whitespace(source_code: &[u8], item_start_index: usize) -> String {
    let mut item_diff_whitespace_start = item_start_index as isize - 1;

    while item_diff_whitespace_start > 0 {
        if source_code[item_diff_whitespace_start as usize].is_ascii_whitespace() {
            if source_code[item_diff_whitespace_start as usize] == ASCII_LINE_FEED {
                break;
            } else {
                item_diff_whitespace_start -= 1;
            }
        } else {
            break;
        }
    }

    // TODO: this omits commas between fields (applies to variants and traits, too)
    if !source_code[item_diff_whitespace_start as usize].is_ascii_whitespace() {
        // we hit a non-whitespace character which shouldn't be included in our output
        item_diff_whitespace_start += 1;
    }

    let whitespace_bytes =
        source_code[item_diff_whitespace_start as usize..item_start_index].to_vec();

    String::from_utf8(whitespace_bytes).expect("Off a code boundary")
}

fn escape_html(input: &str) -> String {
    input
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&#39;")
}

#[cfg(test)]
mod rendering_tests {
    use super::*;
    use std::sync::Mutex;

    static COLOR_OVERRIDE: Mutex<()> = Mutex::new(());

    fn with_colors<T>(enabled: bool, render: impl FnOnce() -> T) -> T {
        let _guard = COLOR_OVERRIDE.lock().unwrap();
        colored::control::set_override(enabled);
        let rendered = render();
        colored::control::set_override(false);
        rendered
    }

    fn overview(path: &str, source: &str) -> Overview {
        Overview::try_from((PathBuf::from(path), source.to_owned())).unwrap()
    }

    fn diff(before: &str, after: &str) -> OverviewDiff {
        overview("before.rs", before).diff_with(&overview("after.rs", after))
    }

    fn assert_highlighted_case(
        label: &str,
        before: &str,
        after: &str,
        section: &str,
        deleted: &[&str],
        added: &[&str],
    ) -> String {
        let overview_diff = diff(before, after);
        assert!(!overview_diff.all_empty(), "{label} produced no diff");

        let html = overview_diff.to_html();
        assert!(
            html.contains(&format!(
                "<details class=\"diff-section\" open><summary>{section}</summary>"
            )),
            "{label} was not rendered in the {section} HTML section"
        );

        let (terminal, styled_section) = with_colors(true, || {
            (overview_diff.to_string(), terminal_header(section))
        });
        assert!(
            terminal.contains(&styled_section),
            "{label} was not rendered in the {section} terminal section: {terminal:?}"
        );

        for expected in deleted {
            let escaped = escape_html(expected);
            assert!(
                html.contains(&format!("<span class=\"deleted\">{escaped}")),
                "{label} did not mark {expected:?} as deleted in HTML: {html}"
            );
            assert!(
                terminal.contains(&format!("\u{1b}[31m{expected}\u{1b}[0m")),
                "{label} did not highlight {expected:?} as deleted in the terminal: {terminal:?}"
            );
        }

        for expected in added {
            let escaped = escape_html(expected);
            assert!(
                html.contains(&format!("<span class=\"added\">{escaped}")),
                "{label} did not mark {expected:?} as added in HTML: {html}"
            );
            assert!(
                terminal.contains(&format!("\u{1b}[32m{expected}\u{1b}[0m")),
                "{label} did not highlight {expected:?} as added in the terminal: {terminal:?}"
            );
        }

        terminal
    }

    #[test]
    fn terminal_renderer_uses_unified_change_lines() {
        let view = ViewableDiffs::new(vec![ViewableDiff {
            old: Some(vec![(
                Some(ExistenceChange::Deleted),
                Code("fn old()".to_owned()),
            )]),
            new: Some(vec![(
                Some(ExistenceChange::Added),
                Code("fn new()".to_owned()),
            )]),
        }]);

        assert_eq!(
            with_colors(false, || view.to_string()),
            "- fn old()\n+ fn new()"
        );
    }

    #[test]
    fn terminal_renderer_highlights_only_changed_spans() {
        let view = ViewableDiffs::new(vec![ViewableDiff {
            old: Some(vec![
                (None, Code("fn ".to_owned())),
                (Some(ExistenceChange::Deleted), Code("old".to_owned())),
                (None, Code("()".to_owned())),
            ]),
            new: Some(vec![
                (None, Code("fn ".to_owned())),
                (Some(ExistenceChange::Added), Code("new".to_owned())),
                (None, Code("()".to_owned())),
            ]),
        }]);

        assert_eq!(
            with_colors(true, || view.to_string()),
            concat!(
                "\u{1b}[31m-\u{1b}[0m fn \u{1b}[31mold\u{1b}[0m()\n",
                "\u{1b}[32m+\u{1b}[0m fn \u{1b}[32mnew\u{1b}[0m()"
            )
        );
    }

    #[test]
    fn terminal_overview_uses_compact_sections_and_keeps_elision_markers() {
        let before = r#"struct Record {
    keep: bool,
    stable: String,
    old: u8,
}
"#;
        let after = r#"struct Record {
    keep: bool,
    stable: String,
    new: u16,
}
"#;

        let header = format!("{:<TERMINAL_HEADER_WIDTH$}", "Structs");
        let expected = format!(
            "before.rs → after.rs\n{header}\n  struct Record {{\n      ..\n-     old: u8\n+     new: u16\n  }}",
        );
        assert_eq!(
            with_colors(false, || diff(before, after).to_string()),
            expected
        );
    }

    #[test]
    fn terminal_renderer_factors_unchanged_function_signature_context() {
        let header = format!("{:<TERMINAL_HEADER_WIDTH$}", "Functions");
        let expected = format!(
            "before.rs → after.rs\n{header}\n  fn new(\n-     revision1: String, revision2: Option<String>\n+     commit1: String, commit2: Option<String>\n  ) -> Self",
        );
        assert_eq!(
            with_colors(false, || {
                diff(
                    "fn new(revision1: String, revision2: Option<String>) -> Self { todo!() }",
                    "fn new(commit1: String, commit2: Option<String>) -> Self { todo!() }",
                )
                .to_string()
            }),
            expected
        );
    }

    #[test]
    fn terminal_renderer_merges_context_inside_a_declaration() {
        let terminal = with_colors(false, || {
            diff(
                r#"enum Command {
    Keep,
    #[doc = "old"]
    Diff {
        old: String,
    },
}"#,
                r#"enum Command {
    Keep,
    #[doc = "new"]
    Diff {
        new: String,
    },
}"#,
            )
            .to_string()
        });

        assert_eq!(terminal.matches("Diff {").count(), 1, "{terminal}");
        assert!(terminal.contains("-     #[doc = \"old\"]"), "{terminal}");
        assert!(terminal.contains("+     #[doc = \"new\"]"), "{terminal}");
        assert!(terminal.contains("-         old: String"), "{terminal}");
        assert!(terminal.contains("+         new: String"), "{terminal}");
    }

    #[test]
    fn renders_every_supported_top_level_item() {
        let cases = [
            (
                "use declarations",
                "use old::Thing;\n",
                "use new::Thing;\n",
                "Uses",
                "use old::Thing",
                "use new::Thing",
            ),
            (
                "struct declarations",
                "struct Removed;\n",
                "struct Added;\n",
                "Structs",
                "struct Removed;",
                "struct Added;",
            ),
            (
                "enum declarations",
                "enum Removed { Variant }\n",
                "enum Added { Variant }\n",
                "Enums",
                "enum Removed { Variant }",
                "enum Added { Variant }",
            ),
            (
                "trait declarations",
                "trait Removed { fn run(&self); }\n",
                "trait Added { fn run(&self); }\n",
                "Traits",
                "trait Removed { fn run(&self); }",
                "trait Added { fn run(&self); }",
            ),
            (
                "free functions",
                "fn removed() {}\n",
                "fn added() {}\n",
                "Functions",
                "fn removed()",
                "fn added()",
            ),
            (
                "impl blocks",
                "struct Old; impl Old { fn run(&self) {} }\n",
                "struct New; impl New { fn run(&self) {} }\n",
                "Impls",
                "impl Old {",
                "impl New {",
            ),
        ];

        for (label, before, after, section, deleted, added) in cases {
            assert_highlighted_case(label, before, after, section, &[deleted], &[added]);
        }
    }

    #[test]
    fn renders_supported_struct_elements_and_preserves_elision() {
        let terminal = assert_highlighted_case(
            "named struct fields, visibility, generics, and where predicates",
            "pub struct Record<T> where T: Copy { keep: bool, stable: u8, old: T }\n",
            "pub(crate) struct Record<T, U> where T: Copy, U: Clone { keep: bool, stable: u8, new: U }\n",
            "Structs",
            &["pub", "<T>", "old: T"],
            &["pub(crate)", "<T, U>", "U: Clone", "new: U"],
        );
        assert!(
            terminal.contains(".."),
            "unchanged fields must remain visibly elided: {terminal:?}"
        );

        assert_highlighted_case(
            "named field modifications",
            "struct Record { value: u8 }\n",
            "struct Record { value: String }\n",
            "Structs",
            &["value: u8"],
            &["value: String"],
        );
        assert_highlighted_case(
            "tuple struct fields",
            "struct Record(pub u8, String);\n",
            "struct Record(pub u16, String, bool);\n",
            "Structs",
            &["pub u8"],
            &["pub u16", "bool"],
        );
        assert_highlighted_case(
            "unit and named struct forms",
            "struct Record;\n",
            "struct Record { value: u8 }\n",
            "Structs",
            &[],
            &["value: u8"],
        );
    }

    #[test]
    fn renders_supported_enum_elements() {
        let terminal = assert_highlighted_case(
            "enum visibility, generics, variants, and fields",
            "pub enum Message<T> { Keep, Stable, Removed, Data { old: T } }\n",
            "pub(crate) enum Message<T, U> { Keep, Stable, Added, Data { new: U } }\n",
            "Enums",
            &["pub", "<T>", "Removed", "old: T"],
            &["pub(crate)", "<T, U>", "Added", "new: U"],
        );
        assert!(terminal.contains(".."), "unchanged variants must be elided");

        assert_highlighted_case(
            "tuple variant fields",
            "enum Message { Data(u8, String) }\n",
            "enum Message { Data(u16, String, bool) }\n",
            "Enums",
            &["u8"],
            &["u16", "bool"],
        );
        assert_highlighted_case(
            "unit and named variant forms",
            "enum Message { Data }\n",
            "enum Message { Data { value: u8 } }\n",
            "Enums",
            &[],
            &["value: u8"],
        );
    }

    #[test]
    fn renders_every_supported_function_signature_element() {
        assert_highlighted_case(
            "function signature components",
            r#"pub const unsafe extern "C" fn calculate<T>(value: T) -> u8 where T: Copy { 0 }"#,
            r#"pub(crate) async extern "Rust" fn calculate<T, U>(value: U, extra: u8) -> u16 where U: Clone { 0 }"#,
            "Functions",
            &[
                "pub",
                "const",
                "unsafe",
                "extern \"C\"",
                "<T>",
                "value: T",
                "-> u8",
                "T: Copy",
            ],
            &[
                "pub(crate)",
                "async",
                "extern \"Rust\"",
                "<T, U>",
                "value: U",
                "extra: u8",
                "-> u16",
                "U: Clone",
            ],
        );
    }

    #[test]
    fn renders_supported_trait_and_impl_elements() {
        assert_highlighted_case(
            "trait visibility, generics, and methods",
            "pub trait Service<T> where T: Copy { fn run(&self, value: T) -> u8; }\n",
            "pub(crate) trait Service<T, U> where U: Clone { fn run(&mut self, value: U) -> u16; fn stop(&self); }\n",
            "Traits",
            &["pub", "<T>", "T: Copy", "&self", "value: T", "-> u8"],
            &[
                "pub(crate)",
                "<T, U>",
                "U: Clone",
                "&mut self",
                "value: U",
                "-> u16",
                "fn stop(&self)",
            ],
        );

        assert_highlighted_case(
            "impl unsafety, generics, and methods",
            "struct Service<T>(T); impl<T> Service<T> where T: Copy { fn run(&self, value: T) -> u8 { 0 } }\n",
            "struct Service<T>(T); unsafe impl<T, U> Service<T> where U: Clone { pub async fn run(&mut self, value: U) -> u16 { 0 } fn stop(&self) {} }\n",
            "Impls",
            &["<T>", "T: Copy", "&self", "value: T", "-> u8"],
            &[
                "unsafe",
                "<T, U>",
                "U: Clone",
                "pub",
                "async",
                "&mut self",
                "value: U",
                "-> u16",
                "fn stop(&self)",
            ],
        );
    }

    #[test]
    fn deleted_impl_is_highlighted_from_its_declaration_in_both_renderers() {
        let before = r#"struct RandomStruct;
impl RandomStruct {
    fn run(&self) {
        println!("body must be elided");
    }
}"#;
        let after = "struct RandomStruct;";
        let overview_diff = diff(before, after);
        let html = overview_diff.to_html();
        let terminal = with_colors(true, || overview_diff.to_string());

        assert!(
            html.contains("<span class=\"deleted\">impl RandomStruct {"),
            "{html}"
        );
        assert!(
            terminal.contains("\u{1b}[31mimpl RandomStruct {\u{1b}[0m"),
            "{terminal:?}"
        );
        assert!(html.contains("fn run(&amp;self)"), "{html}");
        assert!(terminal.contains("fn run(&self)"), "{terminal:?}");
        assert!(!html.contains("body must be elided"), "{html}");
        assert!(!terminal.contains("body must be elided"), "{terminal:?}");
    }

    #[test]
    fn html_renderer_marks_changes_and_escapes_code() {
        let view = ViewableDiffs::new(vec![ViewableDiff {
            old: Some(vec![
                (None, Code("Result<".to_owned())),
                (Some(ExistenceChange::Deleted), Code("A & B".to_owned())),
            ]),
            new: Some(vec![
                (None, Code("Result<".to_owned())),
                (Some(ExistenceChange::Added), Code("A > B".to_owned())),
            ]),
        }]);

        assert_eq!(
            view.to_html(),
            concat!(
                "<tr class=\"diff-item\">",
                "<td class=\"diff-cell deleted-cell\">",
                "<div class=\"diff-cell-scroll\"><pre><code>",
                "<span class=\"\">Result&lt;</span>",
                "<span class=\"deleted\">A &amp; B</span>",
                "</code></pre></div></td>",
                "<td class=\"diff-cell added-cell\">",
                "<div class=\"diff-cell-scroll\"><pre><code>",
                "<span class=\"\">Result&lt;</span>",
                "<span class=\"added\">A &gt; B</span>",
                "</code></pre></div></td></tr>"
            )
        );
    }

    #[test]
    fn html_renderer_uses_empty_cells_for_one_sided_diffs() {
        let addition = ViewableDiffs::new(vec![ViewableDiff {
            old: None,
            new: Some(vec![(
                Some(ExistenceChange::Added),
                Code("struct Added;".to_owned()),
            )]),
        }]);
        let deletion = ViewableDiffs::new(vec![ViewableDiff {
            old: Some(vec![(
                Some(ExistenceChange::Deleted),
                Code("struct Removed;".to_owned()),
            )]),
            new: None,
        }]);

        assert_eq!(
            addition.to_html(),
            concat!(
                "<tr class=\"diff-item\"><td class=\"empty-content\"></td>",
                "<td class=\"diff-cell added-cell\">",
                "<div class=\"diff-cell-scroll\"><pre><code>",
                "<span class=\"added\">struct Added;</span>",
                "</code></pre></div></td></tr>"
            )
        );
        assert_eq!(
            deletion.to_html(),
            concat!(
                "<tr class=\"diff-item\"><td class=\"diff-cell deleted-cell\">",
                "<div class=\"diff-cell-scroll\"><pre><code>",
                "<span class=\"deleted\">struct Removed;</span>",
                "</code></pre></div></td><td class=\"empty-content\"></td></tr>"
            )
        );
    }

    #[test]
    fn html_renderer_does_not_treat_unchanged_context_as_a_removed_item() {
        let addition_with_context = ViewableDiffs::new(vec![ViewableDiff {
            old: Some(vec![(None, Code("enum Command { .. }".to_owned()))]),
            new: Some(vec![
                (None, Code("enum Command { .. ".to_owned())),
                (Some(ExistenceChange::Added), Code("Config".to_owned())),
                (None, Code(" }".to_owned())),
            ]),
        }]);

        assert_eq!(
            addition_with_context.to_html(),
            concat!(
                "<tr class=\"diff-item\"><td class=\"empty-content\"></td>",
                "<td class=\"diff-cell added-cell\">",
                "<div class=\"diff-cell-scroll\"><pre><code>",
                "<span class=\"\">enum Command { .. </span>",
                "<span class=\"added\">Config</span>",
                "<span class=\"\"> }</span>",
                "</code></pre></div></td></tr>"
            )
        );
    }

    #[test]
    fn html_renderer_preserves_collapsed_item_boundaries_without_extra_space() {
        let html = diff("use alpha::A;\nuse beta::B;\n", "").to_html();

        assert!(!html.contains("<span class=\"\">\n</span>"), "{html}");
        assert_eq!(html.matches("<tr class=\"diff-item\">").count(), 2);
    }

    #[test]
    fn html_renderer_gives_changed_functions_separate_visual_items() {
        let before = "fn first() {}\nfn second(value: u8) {}\n";
        let html = diff(before, "").to_html();

        assert_eq!(html.matches("<tr class=\"diff-item\">").count(), 2);
        assert_eq!(html.matches("deleted-cell").count(), 2);
    }

    #[test]
    fn html_renderer_omits_an_unchanged_side_of_an_impl_diff() {
        let before = "impl Widget { fn kept(&self) {} fn removed(&self) {} }";
        let after = "impl Widget { fn kept(&self) {} }";
        let html = diff(before, after).to_html();

        assert_eq!(html.matches("..</span>").count(), 1, "{html}");
        assert_eq!(html.matches("empty-content").count(), 1, "{html}");
    }

    #[test]
    fn overview_html_renders_each_changed_rust_item_in_stable_order() {
        let before = r#"
use old::Thing;

struct Record { old: u8 }
enum Choice { Old }
trait Work { fn run(&self); }
fn top(value: u8) -> u8 { value }
impl Record { fn method(&self) {} }
"#;
        let after = r#"
use new::Thing;

struct Record { new: String }
enum Choice { New(String) }
trait Work { fn run(&mut self, value: u8); }
fn top(value: String) -> String { value }
impl Record { fn method(&mut self, value: u8) {} }
"#;

        let html = diff(before, after).to_html();
        let expected_sections = ["Uses", "Structs", "Enums", "Traits", "Functions", "Impls"];
        let mut previous = 0;
        for section in expected_sections {
            let position = html
                .find(&format!(
                    "<details class=\"diff-section\" open><summary>{section}</summary>"
                ))
                .unwrap_or_else(|| panic!("missing {section} section in {html}"));
            assert!(position >= previous, "{section} was rendered out of order");
            previous = position;
        }

        assert!(
            html.starts_with(
                "<details class=\"file-diff\" data-file-path=\"before.rs\" open><summary>before.rs → after.rs</summary>"
            )
        );
        assert_eq!(html.matches("<colgroup><col><col></colgroup>").count(), 6);
        assert!(html.ends_with("</details>"));
        assert!(html.contains("<span class=\"deleted\">"));
        assert!(html.contains("<span class=\"added\">"));
    }

    #[test]
    fn config_hides_sections_in_terminal_and_html_output() {
        let overview_diff = diff(
            "struct Record { old: u8 }\nenum Choice { Old }",
            "struct Record { new: u8 }\nenum Choice { New }",
        );
        let mut config = Config::default();
        config.enums.show = false;

        let html = overview_diff.to_html_with_config(&config);
        let terminal = with_colors(false, || overview_diff.to_terminal_with_config(&config));

        assert!(html.contains("<summary>Structs</summary>"), "{html}");
        assert!(!html.contains("<summary>Enums</summary>"), "{html}");
        assert!(terminal.contains("Structs"), "{terminal}");
        assert!(!terminal.contains("Enums"), "{terminal}");
    }

    #[test]
    fn config_suppresses_files_with_only_hidden_changes() {
        let overview_diff = diff("enum Choice { Old }", "enum Choice { New }");
        let mut config = Config::default();
        config.enums.show = false;

        assert!(!overview_diff.has_visible_changes(&config));
        assert_eq!(overview_diff.to_html_with_config(&config), "");
        assert_eq!(
            with_colors(false, || overview_diff.to_terminal_with_config(&config)),
            ""
        );
    }

    #[test]
    fn unchanged_overview_has_no_terminal_output() {
        let source = "struct Same { value: u8 }\n";
        let unchanged = diff(source, source);

        assert!(unchanged.all_empty());
        assert_eq!(with_colors(false, || unchanged.to_string()), "");
    }
}
