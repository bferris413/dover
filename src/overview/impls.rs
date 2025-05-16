use std::{collections::HashMap, fmt, ops::Range};

use syn::{
    spanned::Spanned,
    token::{For, Not, Unsafe},
    ImplItem, ItemImpl, Path, Type,
};

use crate::{
    collect_src_maps, overview::functions::Functions, ByteRange, Change, Code, Diff,
    ExistenceChange, SourceFile, View, ViewableDiff, ViewableDiffs,
};

use super::{
    functions::{FunctionsDiff, UnsafeDiff},
    generics::{Generics, GenericsDiff},
};

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
                    let unsafe_diff: Option<UnsafeDiff> = None;
                    let generics_diff:  Option<GenericsDiff> = None;
                    let items_diff = {
                        // TODO: only supports functions
                        let self_fn_items: Vec<_> = impl_
                            .items
                            .iter()
                            .filter_map(|i| match i {
                                ImplItem::Fn(func) => Some(func.clone()),
                                _ => None,
                            })
                            .collect();

                        let other_items_fns = Functions::new_impl(vec![], impl_.source.clone());
                        // TODO: using impl_.source here because I don't have source here, fix this.
                        let self_items_fns = Functions::new_impl(self_fn_items, impl_.source.clone());
                        let fns_diff = self_items_fns.diff_with(&other_items_fns);

                        if fns_diff.is_empty() {
                            None
                        } else {
                            Some(ImplItemsDiff { fns_diff })
                        }
                    };

                    if unsafe_diff.is_none() && generics_diff.is_none() && items_diff.is_none() {
                        continue;
                    } else {
                        // take all the diffs, if old/new exists, get byte range and store in vec
                        let (old_src_map, new_src_map) =
                            collect_src_maps!(unsafe_diff, generics_diff, items_diff,);
                        let diff = ImplDiff {
                            change: Change::Existence(ExistenceChange::Deleted),
                            old: Some(impl_.original.clone()),
                            old_src: Some(impl_.source.clone()),
                            new: None,
                            new_src: None,
                            unsafe_diff,
                            generics_diff,
                            items_diff,
                            old_src_map,
                            new_src_map,
                        };
                        impl_diffs.push(diff)
                    }
                }
            }
        }

        // file2
        // Everything here is either new or already accounted for
        for impl_ in &other.impls {
            if let None = self.impls.iter().find(|i| i.self_ty == impl_.self_ty && i.trait_ == impl_.trait_) {
                // impl was added
                let unsafe_diff: Option<UnsafeDiff> = None;
                let generics_diff:  Option<GenericsDiff> = None;
                let items_diff = {
                    // TODO: only supports functions
                    let self_fn_items: Vec<_> = impl_
                        .items
                        .iter()
                        .filter_map(|i| match i {
                            ImplItem::Fn(func) => Some(func.clone()),
                            _ => None,
                        })
                        .collect();

                    // TODO: using impl_.source here because I don't have source here, fix this.
                    let other_items_fns = Functions::new_impl(vec![], impl_.source.clone());
                    let self_items_fns = Functions::new_impl(self_fn_items, impl_.source.clone());
                    let fns_diff = other_items_fns.diff_with(&self_items_fns);

                    if fns_diff.is_empty() {
                        None
                    } else {
                        Some(ImplItemsDiff { fns_diff })
                    }
                };

                if unsafe_diff.is_none() && generics_diff.is_none() && items_diff.is_none() {
                    continue;
                } else {
                    // take all the diffs, if old/new exists, get byte range and store in vec
                    let (old_src_map, new_src_map) =
                        collect_src_maps!(unsafe_diff, generics_diff, items_diff,);
                    let diff = ImplDiff {
                        change: Change::Existence(ExistenceChange::Added),
                        new: Some(impl_.original.clone()),
                        old: None,
                        unsafe_diff,
                        generics_diff,
                        items_diff,
                        new_src: Some(impl_.source.clone()),
                        old_src: None,
                        old_src_map,
                        new_src_map,
                    };
                    impl_diffs.push(diff)
                }
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
                None
            } else {
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
            }
        };

        if unsafe_diff.is_none() && generics_diff.is_none() && items_diff.is_none() {
            None
        } else {
            // take all the diffs, if old/new exists, get byte range and store in vec
            let (old_src_map, new_src_map) =
                collect_src_maps!(unsafe_diff, generics_diff, items_diff,);
            Some(ImplDiff {
                change: Change::Modified,
                old: Some(self.original.clone()),
                new: Some(other.original.clone()),
                unsafe_diff,
                generics_diff,
                items_diff,
                old_src: Some(self.source.clone()),
                new_src: Some(other.source.clone()),
                old_src_map,
                new_src_map,
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
impl View for ImplsDiff {
    fn as_viewable(&self) -> crate::ViewableDiffs {
        let ex_diffs = self
            .impl_diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Existence(_)));

        let mut viewables = ViewableDiffs::empty();
        for ex_diff in ex_diffs {
            viewables.appendln(ex_diff.as_viewable());
        }

        // add/delete diffs should be side-by-side
        viewables.collapse();

        let mod_diffs = self
            .impl_diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for mod_diff in mod_diffs {
            viewables.appendln(mod_diff.as_viewable());
        }
        
        viewables
    }
}

#[derive(Default, Debug)]
pub struct ImplDiff {
    change: Change,
    old: Option<ItemImpl>,
    new: Option<ItemImpl>,
    #[allow(unused)]
    unsafe_diff: Option<UnsafeDiff>,
    #[allow(unused)]
    generics_diff: Option<GenericsDiff>,
    #[allow(unused)]
    items_diff: Option<ImplItemsDiff>,
    old_src: Option<SourceFile>,
    new_src: Option<SourceFile>,
    old_src_map: Vec<Range<usize>>,
    new_src_map: Vec<Range<usize>>,
}
impl View for ImplDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        let mut old_diff = Vec::new();
        if let Some(old) = self.old.as_ref() {
            let old_src = &self.old_src.as_ref().unwrap().0.as_bytes();

            let old_range = old.span().byte_range();
            let decl_start = old_range.start;
            let decl_end = old.brace_token.span.span().byte_range().start + 1;

            let mut i = decl_start;
            let mut src_i = 0;

            while i < decl_end {
                let maybe_diff_index = self.old_src_map[src_i..]
                    .iter()
                    .position(|r| r.contains(&i));
                match maybe_diff_index {
                    Some(diff_index) => {
                        let diff_range = &self.old_src_map[src_i..][diff_index];

                        // doesn't make sense that we wouldn't be aligned with the start of a range
                        assert_eq!(i, diff_range.start);
                        let substring = old_src[i..diff_range.end].to_vec();
                        let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                        old_diff.push((Some(ExistenceChange::Deleted), code));

                        src_i = diff_index + 1;
                        i = diff_range.end;
                    }
                    None => {
                        let start = i;
                        while i < decl_end {
                            let maybe_diff_index = self.old_src_map[src_i..]
                                .iter()
                                .position(|r| r.contains(&i));
                            if maybe_diff_index.is_some() {
                                break;
                            } else {
                                i += 1
                            }
                        }
                        // We're either off the end or we've found a new diff. Either way,
                        // start..i contains our next range
                        let substring = old_src[start..i].to_vec();
                        let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                        if let Change::Existence(ExistenceChange::Deleted) = self.change {
                            old_diff.push((Some(ExistenceChange::Deleted), code));
                        } else {
                            assert!(matches!(self.change, Change::Modified));
                            old_diff.push((None, code));
                        }
                    }
                }
            }
            
            if let Some(ids) = &self.items_diff {
                let mut i = decl_end;

                while i < old_range.end {
                    let maybe_item_diff = ids.fns_diff.diffs().iter().find(|d| d.old().as_ref().map(|old_func| old_func.original().span().byte_range().contains(&i)).unwrap_or(false));
                    match maybe_item_diff {
                        Some(id) => {
                            let viewable = id.as_viewable();
                            for diff in viewable.vds {
                                if let Some(old) = diff.old {
                                    old_diff.extend(old);
                                }
                            }

                            i = id.old().as_ref().unwrap().original().span().byte_range().end;
                        }
                        None => {
                            if old_src[i].is_ascii_whitespace() {
                                let start = i;
                                while i < old_range.end && old_src[i].is_ascii_whitespace() {
                                    i += 1;
                                }

                                let substring = old_src[start..i].to_vec();
                                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                                old_diff.push((None, code));
                            } else {
                                i += 1;
                            }
                        }
                    }
                }
            }


            if let Change::Existence(ExistenceChange::Deleted) = self.change {
                old_diff.push((Some(ExistenceChange::Deleted), Code("}\n".to_string())));
            } else {
                assert!(matches!(self.change, Change::Modified));
                old_diff.push((None, Code("}\n".to_string())));
            }
        }

        let mut new_diff = Vec::new();
        if let Some(new) = self.new.as_ref() {
            let new_src = &self.new_src.as_ref().unwrap().0.as_bytes();

            let new_range = new.span().byte_range();
            let decl_start = new_range.start;
            let decl_end = new.brace_token.span.span().byte_range().start + 1; // we'll take the "{"

            let mut i = decl_start;
            let mut src_i = 0;

            while i < decl_end {
                let maybe_diff_index = self.new_src_map[src_i..]
                    .iter()
                    .position(|r| r.contains(&i));
                match maybe_diff_index {
                    Some(diff_index) => {
                        let diff_range = &self.new_src_map[src_i..][diff_index];

                        // doesn't make sense that we wouldn't be aligned with the start of a range
                        assert_eq!(i, diff_range.start);
                        let substring = new_src[i..diff_range.end].to_vec();
                        let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                        new_diff.push((Some(ExistenceChange::Added), code));

                        src_i = diff_index + 1;
                        i = diff_range.end;
                    }
                    None => {
                        let start = i;
                        while i < decl_end {
                            let maybe_diff_index = self.new_src_map[src_i..]
                                .iter()
                                .position(|r| r.contains(&i));
                            if maybe_diff_index.is_some() {
                                break;
                            } else {
                                i += 1
                            }
                        }
                        // We're either off the end or we've found a new diff. Either way,
                        // start..i contains our next range
                        let substring = new_src[start..i].to_vec();
                        let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                        if let Change::Existence(ExistenceChange::Added) = self.change {
                            new_diff.push((Some(ExistenceChange::Added), code));
                        } else {
                            assert!(matches!(self.change, Change::Modified));
                            new_diff.push((None, code));
                        }
                    }
                }
            }

            if let Some(ids) = &self.items_diff {
                let mut i = decl_end;
                while i < new_range.end {
                    let maybe_item_diff = ids.fns_diff.diffs().iter().find(|d| d.new().as_ref().map(|new_func| new_func.original().span().byte_range().contains(&i)).unwrap_or(false));
                    match maybe_item_diff {
                        Some(id) => {
                            let viewable = id.as_viewable();
                            for diff in viewable.vds {
                                if let Some(new) = diff.new {
                                    new_diff.extend(new);
                                }
                            }

                            i = id.new().as_ref().unwrap().original().span().byte_range().end;
                        }
                        None => {
                            if new_src[i].is_ascii_whitespace() {
                                let start = i;
                                while i < new_range.end && new_src[i].is_ascii_whitespace() {
                                    i += 1;
                                }

                                let substring = new_src[start..i].to_vec();
                                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                                new_diff.push((None, code));
                            } else {
                                i += 1;
                            }
                        }
                    }
                }
            }


            if let Change::Existence(ExistenceChange::Added) = self.change {
                new_diff.push((Some(ExistenceChange::Added), Code("}\n".to_string())));
            } else {
                assert!(matches!(self.change, Change::Modified));
                new_diff.push((None, Code("}\n".to_string())));
            }
        }

        ViewableDiffs::new(vec![ViewableDiff {
            old: if old_diff.is_empty() { None} else { Some(old_diff) },
            new: if new_diff.is_empty() { None } else { Some(new_diff) },
        }])
    }
}

#[derive(Debug)]
pub struct ImplItemsDiff {
    fns_diff: FunctionsDiff,
}
impl ImplItemsDiff {
    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.fns_diff.diffs().len()
    }
}
impl ByteRange for ImplItemsDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        self.fns_diff.old_ranges()
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        self.fns_diff.new_ranges()
    }
}
