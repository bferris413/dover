use syn::{Abi, ItemFn, ReturnType, Signature};

use crate::{Change, Diff, ExistenceChange, Vis};

use super::generics::Generics;

/// A collection of freestanding `fn` definitions.
///
/// The internal representation is sorted and deduped.
#[derive(Debug, Eq, PartialEq)]
pub struct Functions(pub Vec<Function>);
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
                    };
                    function_diffs.push(fdiff);
                }
            }
        }

        // Everything here is either new or already diffed
        for function in &other.0 {
            if let Err(_e) = self.0.binary_search_by(|s| s.name().cmp(&function.name())) {
                let sdiff = FunctionDiff {
                    change: Change::Existence(ExistenceChange::Added),
                    old: None,
                    new: Some(function.clone()),
                };
                function_diffs.push(sdiff);
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
    r#const: bool,
    r#async: bool,
    r#unsafe: bool,
    abi: Option<Abi>,
    name: String,
    generics: Generics,
    inputs: Inputs,
    output: ReturnType,
    original_sig: Signature,
}
impl Function {
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl From<ItemFn> for Function {
    fn from(item: ItemFn) -> Self {
        Function {
            vis: item.vis.into(),
            r#const: item.sig.constness.is_some(),
            r#async: item.sig.asyncness.is_some(),
            r#unsafe: item.sig.unsafety.is_some(),
            abi: item.sig.abi.clone(),
            name: item.sig.ident.to_string(),
            generics: Generics::from(item.sig.generics.clone()),
            inputs: Inputs {
                args: item.sig.clone().inputs.into_iter().collect(),
            },
            output: item.sig.output.clone(),
            original_sig: item.sig.clone(),
        }
    }
}
impl Diff for Function {
    type Diff = Option<FunctionDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let change = Change::Modified;
        let old = self.clone();
        let new = other.clone();

        Some(FunctionDiff {
            change,
            old: Some(old),
            new: Some(new),
        })
    }
}

pub struct FunctionDiff {
    change: Change,
    old: Option<Function>,
    new: Option<Function>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inputs {
    pub args: Vec<syn::FnArg>,
}

pub struct FunctionsDiff {
    diffs: Vec<FunctionDiff>,
}
