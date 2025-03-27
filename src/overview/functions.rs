use std::fmt::{Display, Formatter};

use quote::ToTokens;
use syn::{
    token::{Async, Const, Unsafe},
    Abi, FnArg, Item, ItemFn, ReturnType,
};

use crate::{get_source, Change, Diff, ExistenceChange, Vis, VisDiff};

use super::generics::{Generics, GenericsDiff};

/// A collection of freestanding `fn` definitions.
///
/// The internal representation is sorted and deduped.
#[derive(Debug, Eq, PartialEq)]
pub struct Functions(Vec<Function>);
impl Functions {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn functions(&self) -> &[Function] {
        &self.0
    }
}
impl From<Vec<ItemFn>> for Functions {
    /// Creates a complete set of freestanding `Function` declarations from a list of `syn::ItemFn`.
    fn from(fns: Vec<ItemFn>) -> Self {
        let mut functions: Vec<Function> =
            fns.into_iter().map(|item| Function::from(item)).collect();
        functions.sort_by(|f1, f2| f1.name().cmp(&f2.name()));
        functions.dedup_by(|f1, f2| f1.name() == f2.name());
        Functions(functions)
    }
}
impl Diff for Functions {
    type Diff = FunctionsDiff;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return FunctionsDiff { diffs: Vec::new() };
        }

        debug_assert!(self.0.is_sorted_by(|s1, s2| s1.name() <= s2.name()));
        debug_assert!(other.0.is_sorted_by(|s1, s2| s1.name() <= s2.name()));

        let mut function_diffs = Vec::with_capacity(usize::max(self.0.len(), other.0.len()));

        for function in &self.0 {
            match other.0.binary_search_by(|s| s.name().cmp(&function.name())) {
                Ok(s) => {
                    if let Some(diff) = function.diff_with(&other.0[s]) {
                        function_diffs.push(diff);
                    }
                }
                Err(_e) => {
                    // function was deleted
                    let fdiff = FunctionDiff {
                        change: Change::Existence(ExistenceChange::Deleted),
                        old: Some(function.clone()),
                        new: None,
                        ..Default::default()
                    };
                    function_diffs.push(fdiff);
                }
            }
        }

        // Everything here is either new or already diffed
        for function in &other.0 {
            if let Err(_e) = self.0.binary_search_by(|s| s.name().cmp(&function.name())) {
                let fdiff = FunctionDiff {
                    change: Change::Existence(ExistenceChange::Added),
                    old: None,
                    new: Some(function.clone()),
                    ..Default::default()
                };
                function_diffs.push(fdiff);
            }
        }

        FunctionsDiff {
            diffs: function_diffs,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    vis: Vis,
    r#const: Option<Const>,
    r#async: Option<Async>,
    r#unsafe: Option<Unsafe>,
    abi: Option<Abi>,
    name: String,
    generics: Generics,
    inputs: Inputs,
    output: ReturnType,
    original_fn: ItemFn,
}
impl Function {
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let source = remove_block(get_source(vec![Item::Fn(self.original_fn.clone())]));
        write!(f, "{source}")
    }
}
impl From<ItemFn> for Function {
    fn from(item: ItemFn) -> Self {
        Function {
            vis: item.vis.clone().into(),
            r#const: item.sig.constness,
            r#async: item.sig.asyncness,
            r#unsafe: item.sig.unsafety,
            abi: item.sig.abi.clone(),
            name: item.sig.ident.to_string(),
            generics: Generics::from(item.sig.generics.clone()),
            inputs: Inputs {
                args: item.sig.clone().inputs.into_iter().collect(),
            },
            output: item.sig.output.clone(),
            original_fn: item.clone(),
        }
    }
}
impl Diff for Function {
    type Diff = Option<FunctionDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let vis_diff = self.vis.diff_with(&other.vis);
        let const_diff = self.r#const.diff_with(&other.r#const);
        let async_diff = self.r#async.diff_with(&other.r#async);
        let unsafe_diff = self.r#unsafe.diff_with(&other.r#unsafe);
        let abi_diff = self.abi.diff_with(&other.abi);
        let generics_diff = self.generics.diff_with(&other.generics);
        let inputs_diff = self.inputs.diff_with(&other.inputs);
        let return_type_diff = self.output.diff_with(&other.output);

        if vis_diff.is_none()
            && const_diff.is_none()
            && async_diff.is_none()
            && unsafe_diff.is_none()
            && abi_diff.is_none()
            && generics_diff.is_none()
            && inputs_diff.is_none()
            && return_type_diff.is_none()
        {
            return None;
        }

        Some(FunctionDiff {
            name: self.name.clone(),
            change: Change::Modified,
            vis_diff,
            const_diff,
            async_diff,
            unsafe_diff,
            abi_diff,
            generics_diff,
            inputs_diff,
            return_type_diff,
            old: Some(self.clone()),
            new: Some(other.clone()),
        })
    }
}

#[derive(Debug, Default)]
pub struct FunctionDiff {
    #[allow(unused)]
    name: String,
    change: Change,

    vis_diff: Option<VisDiff>,
    const_diff: Option<ConstDiff>,
    async_diff: Option<AsyncDiff>,
    unsafe_diff: Option<UnsafeDiff>,
    abi_diff: Option<AbiDiff>,
    generics_diff: Option<GenericsDiff>,
    inputs_diff: Option<InputsDiff>,
    return_type_diff: Option<ReturnTypeDiff>,
    old: Option<Function>,
    new: Option<Function>,
}

impl Display for FunctionDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Change::Existence(ex) = self.change {
            let func = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new functions were both Some"),
                (None, None) => panic!("old and new functions were both None"),
                (Some(s), None) | (None, Some(s)) => s,
            };

            return write!(f, "{ex} {func}");
        }

        let mut left_column = Vec::new();
        let mut right_column = Vec::new();

        // old and new function declarations
        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = format!("{old}");
        let new_source = format!("{new}");
        left_column.push(old_source);
        right_column.push(new_source);

        if let Some(abi_diff) = &self.abi_diff {
            left_column.push("\nabi:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", abi_diff.old));
            right_column.push(format!("+ {}", abi_diff.new));
        }

        // old and new visibility modifiers, if any
        if let Some(vd) = &self.vis_diff {
            left_column.push("\nvisibility:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", vd.old));
            right_column.push(format!("+ {}", vd.new));
        }

        // old and new const modifiers, if any
        if let Some(cd) = &self.const_diff {
            left_column.push("\nconst:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", cd.old));
            right_column.push(format!("+ {}", cd.new));
        }

        // old and new async modifiers, if any
        if let Some(cd) = &self.async_diff {
            left_column.push("\nasync:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", cd.old));
            right_column.push(format!("+ {}", cd.new));
        }

        // old and new unsafe modifiers, if any
        if let Some(cd) = &self.unsafe_diff {
            left_column.push("\nunsafe:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", cd.old));
            right_column.push(format!("+ {}", cd.new));
        }

        // old and new generics, if any
        if let Some(gd) = &self.generics_diff {
            // generic param diff, if any
            if let Some(pd) = gd.params_diff() {
                let mut old_params = Vec::new();
                let mut new_params = Vec::new();

                for pd in pd.iter() {
                    let param_source = pd.param().unwrap().to_token_stream().to_string();
                    // let param_source = get_source(vec![Item::Verbatim(param_tokens)]);
                    match pd.change() {
                        ExistenceChange::Deleted => old_params.push(format!("- {param_source}",)),
                        ExistenceChange::Added => new_params.push(format!("+ {param_source}",)),
                    }
                }

                left_column.push("\ngeneric parameters:".to_string());
                right_column.push(String::new());
                left_column.push(old_params.join("\n"));
                right_column.push(new_params.join("\n"));
            }

            // where clause diff, if any
            if let Some(wd) = gd.where_diff() {
                left_column.push("\nwhere clause:".to_string());
                right_column.push(String::new());
                match wd.change() {
                    Change::Existence(ex) => {
                        // where clause was added or deleted wholesale
                        let where_clause_source =
                            wd.where_clause().unwrap().to_token_stream().to_string();
                        // let where_clause_source = get_source(vec![Item::Verbatim(where_clause)]);
                        match ex {
                            ExistenceChange::Deleted => {
                                left_column.push(format!("- {where_clause_source}"));
                                right_column.push(String::new());
                            }
                            ExistenceChange::Added => {
                                right_column.push(format!("+ {where_clause_source}"));
                                left_column.push(String::new());
                            }
                        }
                    }
                    Change::Modified => {
                        // where clause predicates were added or deleted
                        let predicate_diffs = wd.predicates().unwrap();
                        let mut old_predicates = Vec::new();
                        let mut new_predicates = Vec::new();

                        for pred_diff in predicate_diffs.iter() {
                            let predicate_source =
                                pred_diff.predicate().unwrap().to_token_stream().to_string();
                            // let predicate_source =
                            //     get_source(vec![Item::Verbatim(predicate_tokens)]);
                            match pred_diff.change() {
                                ExistenceChange::Deleted => {
                                    old_predicates.push(format!("- {predicate_source}",))
                                }
                                ExistenceChange::Added => {
                                    new_predicates.push(format!("+ {predicate_source}",))
                                }
                            }
                        }

                        left_column.push(old_predicates.join("\n"));
                        right_column.push(new_predicates.join("\n"));
                    }
                }
            }
        }

        if let Some(input_diff) = &self.inputs_diff {
            let mut old_inputs = Vec::new();
            let mut new_inputs = Vec::new();

            for id in input_diff.diffs() {
                if let Some(id) = id {
                    match id.change() {
                        Change::Existence(ex) => {
                            // input was added or deleted wholesale
                            match ex {
                                ExistenceChange::Deleted => {
                                    let input_source =
                                        id.old().unwrap().to_token_stream().to_string();
                                    old_inputs.push(format!("- {input_source}",))
                                }
                                ExistenceChange::Added => {
                                    let input_source =
                                        id.new().unwrap().to_token_stream().to_string();
                                    new_inputs.push(format!("+ {input_source}",))
                                }
                            }
                        }
                        Change::Modified => {
                            // input was modified
                            let old_input_source = id.old().unwrap().to_token_stream().to_string();
                            let new_input_source = id.new().unwrap().to_token_stream().to_string();
                            old_inputs.push(format!("- {old_input_source}",));
                            new_inputs.push(format!("+ {new_input_source}",));
                        }
                    }
                }
            }

            left_column.push("\ninputs:".to_string());
            right_column.push(String::new());
            left_column.push(old_inputs.join("\n"));
            right_column.push(new_inputs.join("\n"));
        }

        if let Some(rt_diff) = &self.return_type_diff {
            left_column.push("\nreturn type:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", rt_diff.old().to_token_stream().to_string()));
            right_column.push(format!("+ {}", rt_diff.new().to_token_stream().to_string()));
        }

        let formatted_output = crate::format_as_columns(&left_column, &right_column);
        write!(f, "{formatted_output}")
    }
}

fn remove_block(mut source: String) -> String {
    let block_start = source.find("{");
    let block_end = source.rfind("}");

    if let (Some(start), Some(_)) = (block_start, block_end) {
        source.truncate(start);
        source
    } else {
        eprintln!("unexpected function without block");
        source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inputs {
    pub args: Vec<syn::FnArg>,
}
impl Diff for Inputs {
    type Diff = Option<InputsDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let max_diff = self.args.len().max(other.args.len());
        let mut diffs = Vec::with_capacity(max_diff);
        let mut i = 0;
        loop {
            match (self.args.get(i), other.args.get(i)) {
                (Some(arg1), Some(arg2)) => {
                    diffs.push(arg1.diff_with(arg2));
                }
                (Some(arg1), None) => {
                    diffs.push(Some(FnArgDiff {
                        old: Some(arg1.clone()),
                        new: None,
                        change: Change::Existence(ExistenceChange::Deleted),
                    }));
                }
                (None, Some(arg2)) => {
                    diffs.push(Some(FnArgDiff {
                        new: Some(arg2.clone()),
                        old: None,
                        change: Change::Existence(ExistenceChange::Added),
                    }));
                }
                (None, None) => break,
            }

            i += 1;
        }

        Some(InputsDiff { inputs: diffs })
    }
}

#[derive(Debug)]
pub struct FnArgDiff {
    change: Change,
    old: Option<FnArg>,
    new: Option<FnArg>,
}
impl FnArgDiff {
    fn change(&self) -> Change {
        self.change
    }

    fn old(&self) -> Option<&FnArg> {
        self.old.as_ref()
    }

    fn new(&self) -> Option<&FnArg> {
        self.new.as_ref()
    }
}
impl Diff for FnArg {
    type Diff = Option<FnArgDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            None
        } else {
            Some(FnArgDiff {
                old: Some(self.clone()),
                new: Some(other.clone()),
                change: Change::Modified,
            })
        }
    }
}

#[derive(Debug)]
pub struct FunctionsDiff {
    diffs: Vec<FunctionDiff>,
}
impl FunctionsDiff {
    pub(crate) fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }
}
impl Display for FunctionsDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.diffs.is_empty() {
            return writeln!(f, "(no changes)");
        }

        for diff in self.diffs.iter() {
            writeln!(f, "{diff}")?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct InputsDiff {
    inputs: Vec<Option<FnArgDiff>>,
}
impl InputsDiff {
    fn diffs(&self) -> &[Option<FnArgDiff>] {
        &self.inputs
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstDiff {
    existence: ExistenceChange,
    old: String,
    new: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsyncDiff {
    existence: ExistenceChange,
    old: String,
    new: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsafeDiff {
    existence: ExistenceChange,
    old: String,
    new: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiDiff {
    change: Change,
    old: String,
    new: String,
}

#[derive(Debug)]
pub struct ReturnTypeDiff {
    old: ReturnType,
    new: ReturnType,
}
impl ReturnTypeDiff {
    fn old(&self) -> &ReturnType {
        &self.old
    }

    fn new(&self) -> &ReturnType {
        &self.new
    }
}

impl Diff for ReturnType {
    type Diff = Option<ReturnTypeDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            None
        } else {
            Some(ReturnTypeDiff {
                old: self.clone(),
                new: other.clone(),
            })
        }
    }
}

impl Diff for Option<Unsafe> {
    type Diff = Option<UnsafeDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        match (self, other) {
            (Some(_), Some(_)) => None,
            (None, Some(_)) => Some(UnsafeDiff {
                existence: ExistenceChange::Added,
                old: "(none)".to_string(),
                new: "unsafe".to_string(),
            }),
            (Some(_), None) => Some(UnsafeDiff {
                existence: ExistenceChange::Deleted,
                old: "unsafe".to_string(),
                new: "(none)".to_string(),
            }),
            (None, None) => None,
        }
    }
}

impl Diff for Option<Async> {
    type Diff = Option<AsyncDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        match (self, other) {
            (Some(_), Some(_)) => None,
            (None, Some(_)) => Some(AsyncDiff {
                existence: ExistenceChange::Added,
                old: "(none)".to_string(),
                new: "async".to_string(),
            }),
            (Some(_), None) => Some(AsyncDiff {
                existence: ExistenceChange::Deleted,
                old: "async".to_string(),
                new: "(none)".to_string(),
            }),
            (None, None) => None,
        }
    }
}

impl Diff for Option<Const> {
    type Diff = Option<ConstDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        match (self, other) {
            (Some(_), Some(_)) => None,
            (None, Some(_)) => Some(ConstDiff {
                existence: ExistenceChange::Added,
                old: "(none)".to_string(),
                new: "const".to_string(),
            }),
            (Some(_), None) => Some(ConstDiff {
                existence: ExistenceChange::Deleted,
                old: "const".to_string(),
                new: "(none)".to_string(),
            }),
            (None, None) => None,
        }
    }
}

impl Diff for Option<Abi> {
    type Diff = Option<AbiDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        match (self, other) {
            (Some(old), Some(new)) => Some(AbiDiff {
                change: Change::Modified,
                old: format!("extern \"{}\"", old.name.as_ref().unwrap().value()),
                new: format!("extern \"{}\"", new.name.as_ref().unwrap().value()),
            }),
            (None, Some(new)) => Some(AbiDiff {
                change: Change::Existence(ExistenceChange::Added),
                old: "(none)".to_string(),
                new: format!("extern \"{}\"", new.name.as_ref().unwrap().value()),
            }),
            (Some(old), None) => Some(AbiDiff {
                change: Change::Existence(ExistenceChange::Deleted),
                old: format!("extern \"{}\"", old.name.as_ref().unwrap().value()),
                new: "(none)".to_string(),
            }),
            (None, None) => None,
        }
    }
}
