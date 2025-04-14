use std::{
    collections::HashMap,
    fmt::{self, Display, Write},
};

use syn::{
    spanned::Spanned,
    token::{For, Not, Unsafe},
    ImplItem, ItemImpl, Path, Type,
};

use crate::{overview::functions::Functions, Change, Diff, ExistenceChange, SourceFile};

use super::{
    functions::{FunctionsDiff, UnsafeDiff},
    generics::{Generics, GenericsDiff},
};

const NO_SRC_ERROR: &str = "No source text for impl, was parse logic changed?";

#[derive(Debug)]
pub struct Impls {
    impls: Vec<Impl>,
}
impl Impls {
    pub fn new(impls: Vec<ItemImpl>, source: SourceFile) -> Self {
        // This is somewhat incorrect because it doesn't consider impls in submodules of
        // the same file, but we merge impls for now to make diffing more straightforward.

        let mut merged_impls = HashMap::new();
        for mut impl_ in impls.into_iter() {
            let impl_key = (impl_.trait_.clone(), impl_.self_ty.clone());

            if impl_.trait_.is_none() {
                merged_impls
                    .entry(impl_key)
                    .and_modify(|existing_impl: &mut ItemImpl| {
                        existing_impl.items.append(&mut impl_.items);
                    })
                    .or_insert(impl_);
            } else {
                let overwrote_impl = merged_impls.insert(impl_key, impl_);
                if let Some(impl_) = overwrote_impl {
                    println!(
                        "SCARY: overwrote trait impl: '{:?}' for {:?}",
                        impl_.trait_.unwrap(),
                        impl_.self_ty
                    );
                }
            }
        }
        let impls = merged_impls
            .into_iter()
            .map(|(_, impl_)| Impl::new(impl_, source.clone()))
            .collect();

        Impls { impls }
    }
}
impl Diff for Impls {
    type Diff = ImplsDiff;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        // PERF: This will be slow for large-ish impls (they're unsorted, unlike the other collection structs)
        let mut impl_diffs = Vec::new();

        // file1
        for impl_ in &self.impls {
            // We don't (currently) delineate between multiple non-trait impls in the same file. In one
            // sense it's incorrect, but in another sense we're just getting a general overview anyways
            let impl_with_matching_type = other
                .impls
                .iter()
                .find(|i| i.self_ty == impl_.self_ty && i.trait_ == impl_.trait_);

            match impl_with_matching_type {
                Some(i) => {
                    if let Some(diff) = impl_.diff_with(&i) {
                        impl_diffs.push(diff);
                    }
                }

                None => {
                    // impl was deleted
                    let impl_diff = ImplDiff {
                        change: Change::Existence(ExistenceChange::Deleted),
                        old: Some(impl_.original.clone()),
                        new: None,
                        unsafe_diff: None,
                        items_diff: None,
                        generics_diff: None,
                    };
                    impl_diffs.push(impl_diff);
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for impl_ in &other.impls {
            if let None = self.impls.iter().find(|i| i.self_ty == impl_.self_ty) {
                let sdiff = ImplDiff {
                    change: Change::Existence(ExistenceChange::Added),
                    old: None,
                    new: Some(impl_.original.clone()),
                    unsafe_diff: None,
                    items_diff: None,
                    generics_diff: None,
                };
                impl_diffs.push(sdiff);
            }
        }

        ImplsDiff { impl_diffs }
    }
}
impl Impls {
    pub(crate) fn is_empty(&self) -> bool {
        self.impls.is_empty()
    }
    pub(crate) fn impls(&self) -> &[Impl] {
        self.impls.as_slice()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Impl {
    original: ItemImpl,
    unsafety: Option<Unsafe>,
    generics: Generics,
    trait_: Option<(Option<Not>, Path, For)>,
    self_ty: Box<Type>,
    items: Vec<ImplItem>,
    source: SourceFile,
}
impl Impl {
    pub fn new(i: ItemImpl, source: SourceFile) -> Self {
        let unsafety = i.unsafety.clone();
        let generics = Generics::from(i.generics.clone());
        let trait_ = i.trait_.clone();
        let self_ty = i.self_ty.clone();
        let items = i.items.clone();
        Impl {
            original: i,
            unsafety,
            generics,
            trait_,
            self_ty,
            items,
            source,
        }
    }
}
impl Diff for Impl {
    type Diff = Option<ImplDiff>;

    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        if self.self_ty != other.self_ty {
            // impls for two different types
            return None;
        }

        if self.trait_ != other.trait_ {
            // impls for two different traits on the same type
            return None;
        }

        let unsafe_diff = self.unsafety.diff_with(&other.unsafety);
        let generics_diff = self.generics.diff_with(&other.generics);
        let items_diff = {
            // TODO: only supports functions
            if self.items == other.items {
                return None;
            }

            let self_fn_items: Vec<_> = self
                .items
                .iter()
                .filter_map(|i| match i {
                    ImplItem::Fn(func) => Some(func.clone()),
                    _ => None,
                })
                .collect();
            let other_fn_items: Vec<_> = other
                .items
                .iter()
                .filter_map(|i| match i {
                    ImplItem::Fn(func) => Some(func.clone()),
                    _ => None,
                })
                .collect();

            let self_items_fns = Functions::new_impl(self_fn_items, self.source.clone());
            let other_items_fns = Functions::new_impl(other_fn_items, other.source.clone());

            let fns_diff = self_items_fns.diff_with(&other_items_fns);
            if fns_diff.is_empty() {
                None
            } else {
                Some(ImplItemsDiff { fns_diff })
            }
        };

        if unsafe_diff.is_none() && generics_diff.is_none() && items_diff.is_none() {
            None
        } else {
            Some(ImplDiff {
                change: Change::Modified,
                old: Some(self.original.clone()),
                new: Some(other.original.clone()),
                unsafe_diff,
                generics_diff,
                items_diff,
            })
        }
    }
}
impl fmt::Display for Impl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(impl)")
    }
}

pub struct ImplsDiff {
    impl_diffs: Vec<ImplDiff>,
}
impl ImplsDiff {
    pub fn is_empty(&self) -> bool {
        self.impl_diffs.is_empty()
    }
}
impl Display for ImplsDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.impl_diffs.is_empty() {
            return writeln!(f, "(no changes)");
        }

        let ex_diffs = self
            .impl_diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Existence(_)));

        let (mut left_col, mut right_col) = (String::new(), String::new());
        let mut any_ex_diffs = false;
        for diff in ex_diffs {
            any_ex_diffs = true;
            match diff.change {
                Change::Existence(ExistenceChange::Added) => {
                    write!(right_col, "{diff}")?;
                }
                Change::Existence(ExistenceChange::Deleted) => {
                    write!(left_col, "{diff}")?;
                }
                _ => {
                    unreachable!()
                }
            }
        }

        if any_ex_diffs {
            let output = crate::format_as_columns(&vec![left_col], &vec![right_col]);
            writeln!(f, "{output}")?;
        }

        let mod_diffs = self
            .impl_diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for diff in mod_diffs {
            writeln!(f, "{diff}")?;
        }

        Ok(())
    }
}

pub struct ImplDiff {
    change: Change,
    old: Option<ItemImpl>,
    new: Option<ItemImpl>,
    unsafe_diff: Option<UnsafeDiff>,
    generics_diff: Option<GenericsDiff>,
    items_diff: Option<ImplItemsDiff>,
}
impl Display for ImplDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Change::Existence(ex) = self.change {
            let i = match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new impls were both Some"),
                (None, None) => panic!("old and new impls were both None"),
                (Some(i), None) | (None, Some(i)) => i,
            };

            let source = i
                .span()
                .source_text()
                .expect(NO_SRC_ERROR)
                .lines()
                .map(|line| format!("{ex} {line}"))
                .collect::<Vec<String>>()
                .join("\n");

            return write!(f, "{source}");
        }

        let mut left_column = Vec::new();
        let mut right_column = Vec::new();

        // old and new impl blocks
        let old = self.old.as_ref().unwrap();
        let new = self.new.as_ref().unwrap();
        let old_source = old
            .span()
            .source_text()
            .expect(NO_SRC_ERROR)
            .lines()
            .map(|line| format!("~ {line}"))
            .collect::<Vec<String>>()
            .join("\n");
        let new_source = new
            .span()
            .source_text()
            .expect(NO_SRC_ERROR)
            .lines()
            .map(|line| format!("~ {line}"))
            .collect::<Vec<String>>()
            .join("\n");
        left_column.push(old_source);
        right_column.push(new_source);

        // old and new unsafe modifiers, if any
        if let Some(ud) = &self.unsafe_diff {
            left_column.push("\nunsafe:".to_string());
            right_column.push(String::new());
            left_column.push(format!("- {}", ud.old));
            right_column.push(format!("+ {}", ud.new));
        }

        // generics
        if let Some(gd) = &self.generics_diff {
            // generic param diff, if any
            if let Some(pd) = gd.params_diff() {
                let mut old_params = Vec::new();
                let mut new_params = Vec::new();

                for pd in pd.iter() {
                    let param_source = pd
                        .param()
                        .unwrap()
                        .span()
                        .source_text()
                        .expect(NO_SRC_ERROR);
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
                        let where_clause_source = wd
                            .where_clause()
                            .unwrap()
                            .span()
                            .source_text()
                            .expect(NO_SRC_ERROR);
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
                            let predicate_source = pred_diff
                                .predicate()
                                .unwrap()
                                .span()
                                .source_text()
                                .expect(NO_SRC_ERROR);
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

        let impls_fns_diff = self
            .items_diff
            .as_ref()
            .map(|id| format!("{}", id.fns_diff));

        let formatted_output = crate::format_as_columns(&left_column, &right_column);
        if let Some(diff_output) = impls_fns_diff {
            write!(f, "{formatted_output}\n{diff_output}")
        } else {
            write!(f, "{formatted_output}")
        }
    }
}

pub struct ImplItemsDiff {
    fns_diff: FunctionsDiff,
}
