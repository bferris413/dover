use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use syn::{
    token::{For, Not, Unsafe},
    ImplItem, ItemImpl, Path, Type,
};

use crate::{overview::functions::Functions, Change, Diff, ExistenceChange};

use super::{
    functions::{FunctionsDiff, UnsafeDiff},
    generics::{Generics, GenericsDiff},
};

#[derive(Debug)]
pub struct Impls {
    impls: Vec<Impl>,
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
impl From<Vec<ItemImpl>> for Impls {
    fn from(impls: Vec<ItemImpl>) -> Self {
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
            .map(|(_, impl_)| Impl::from(impl_))
            .collect();

        Impls { impls }
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
        let items_diff = self.items.diff_with(&other.items);

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
impl From<ItemImpl> for Impl {
    fn from(original: ItemImpl) -> Self {
        let unsafety = original.unsafety.clone();
        let generics = Generics::from(original.generics.clone());
        let trait_ = original.trait_.clone();
        let self_ty = original.self_ty.clone();
        let items = original.items.clone();
        Impl {
            original,
            unsafety,
            generics,
            trait_,
            self_ty,
            items,
        }
    }
}
impl Impl {
    pub fn original(&self) -> &ItemImpl {
        &self.original
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
        todo!()
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

impl Diff for Vec<ImplItem> {
    type Diff = Option<ImplItemsDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        // TODO: only supports functions
        if self == other {
            return None;
        }

        let self_fn_items: Vec<_> = self
            .iter()
            .filter_map(|i| match i {
                ImplItem::Fn(func) => Some(func.clone()),
                _ => None,
            })
            .collect();
        let other_fn_items: Vec<_> = self
            .iter()
            .filter_map(|i| match i {
                ImplItem::Fn(func) => Some(func.clone()),
                _ => None,
            })
            .collect();

        let self_fns = Functions::from(self_fn_items);
        let other_fns = Functions::from(other_fn_items);

        let fns_diff = self_fns.diff_with(&other_fns);
        if fns_diff.is_empty() {
            None
        } else {
            Some(ImplItemsDiff { fns_diff })
        }
    }
}

pub struct ImplItemsDiff {
    fns_diff: FunctionsDiff,
}
