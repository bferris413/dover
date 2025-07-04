use std::{collections::HashMap, fmt, ops::Range};

use syn::{
    ImplItem, ItemImpl, Path, Type,
    spanned::Spanned,
    token::{For, Not, Unsafe},
};

use crate::{
    ASCII_LINE_FEED, ByteRange, Change, Code, Diff, ExistenceChange, SourceFile, View,
    ViewableDiff, ViewableDiffs, collect_src_maps,
    overview::functions::{Function, FunctionDiff, Functions},
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
                    let generics_diff: Option<GenericsDiff> = None;
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
                        let self_items_fns =
                            Functions::new_impl(self_fn_items, impl_.source.clone());
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
            if let None = self
                .impls
                .iter()
                .find(|i| i.self_ty == impl_.self_ty && i.trait_ == impl_.trait_)
            {
                // impl was added
                let unsafe_diff: Option<UnsafeDiff> = None;
                let generics_diff: Option<GenericsDiff> = None;
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
            viewables.append(ex_diff.as_viewable());
        }

        // add/delete diffs should be side-by-side
        viewables.collapse();

        let mod_diffs = self
            .impl_diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for mod_diff in mod_diffs {
            viewables.append(mod_diff.as_viewable());
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
            old_diff = collect_impl_diff_changes(
                old,
                &self.old_src.as_ref().unwrap().0.as_bytes(),
                &self.old_src_map,
                ExistenceChange::Deleted,
                match self.change {
                    Change::Existence(ex) => dbg!(Some(ex)),
                    Change::Modified => None,
                },
                &self.items_diff,
            );
        }

        let mut new_diff = Vec::new();
        if let Some(new) = self.new.as_ref() {
            new_diff = collect_impl_diff_changes(
                new,
                &self.new_src.as_ref().unwrap().0.as_bytes(),
                &self.new_src_map,
                ExistenceChange::Added,
                match self.change {
                    Change::Existence(ex) => dbg!(Some(ex)),
                    Change::Modified => None,
                },
                &self.items_diff,
            );
        }

        ViewableDiffs::new(vec![ViewableDiff {
            old: if old_diff.is_empty() {
                None
            } else {
                Some(old_diff)
            },
            new: if new_diff.is_empty() {
                None
            } else {
                Some(new_diff)
            },
        }])
    }
}

fn collect_impl_diff_changes(
    impl_: &ItemImpl,
    source_code: &[u8],
    source_map: &[Range<usize>],
    change_for_diffs: ExistenceChange,
    maybe_change_for_diffs: Option<ExistenceChange>,
    items_diff: &Option<ImplItemsDiff>,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let impl_range = impl_.span().byte_range();
    let decl_start = impl_range.start;
    let sig_end = impl_.brace_token.span.span().byte_range().start + 1; // we'll take the "{"
    let items_end = end_index_of_items(impl_, source_code);

    let mut i = decl_start;
    let mut src_i = 0;
    let mut diffs = Vec::new();

    while i < sig_end {
        let maybe_diff_index = source_map[src_i..].iter().position(|r| r.contains(&i));
        match maybe_diff_index {
            Some(diff_index) => {
                let diff_range = &source_map[src_i..][diff_index];

                // doesn't make sense that we wouldn't be aligned with the start of a range
                assert_eq!(i, diff_range.start);
                let substring = source_code[i..diff_range.end].to_vec();
                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));
                diffs.push((Some(change_for_diffs), code));

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

                diffs.push((maybe_change_for_diffs, code));
            }
        }
    }

    if let Some(ids) = items_diff {
        let (get_orig_field, get_sub_diff): (
            Box<dyn Fn(&FunctionDiff) -> &Option<Function>>,
            Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
        );
        match change_for_diffs {
            ExistenceChange::Added => {
                get_orig_field = Box::new(|fd: &FunctionDiff| fd.new());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.new);
            }
            ExistenceChange::Deleted => {
                get_orig_field = Box::new(|fd: &FunctionDiff| fd.old());
                get_sub_diff = Box::new(|vd: ViewableDiff| vd.old);
            }
        };
        let item_diff_changes = collect_item_diffs(
            source_code,
            &impl_range,
            sig_end,
            ids,
            get_orig_field,
            get_sub_diff,
        );
        diffs.extend(item_diff_changes);
    }

    // collect remaining whitespace and closing ')' or '}'
    let code = String::from_utf8(source_code[items_end..impl_range.end].to_vec())
        .expect("Off a code boundary");

    diffs.push((maybe_change_for_diffs, Code(code)));
    diffs
}

fn collect_item_diffs(
    // The full source code for the file we're parsing
    source_code: &[u8],
    // The byte range in the source code of the impl we're parsing
    impl_range: &Range<usize>,
    // The index at which the signature ends
    sig_end: usize,
    // The item diffs for the file we're parsing
    ids: &ImplItemsDiff,
    // How to get the original function from a item diff (old or new method)
    get_original_item: Box<dyn Fn(&FunctionDiff) -> &Option<Function>>,
    // How to get sub diffs from a given viewable diff (old or new field)
    get_sub_diffs: Box<dyn Fn(ViewableDiff) -> Option<Vec<(Option<ExistenceChange>, Code)>>>,
) -> Vec<(Option<ExistenceChange>, Code)> {
    let mut i = sig_end;
    let mut diffs = Vec::new();

    while i < impl_range.end {
        let maybe_item_diff = ids.fns_diff.diffs().iter().find(|d| {
            get_original_item(d)
                .as_ref()
                .map(|func| func.original().span().byte_range().contains(&i))
                .unwrap_or(false)
        });
        match maybe_item_diff {
            Some(id) => {
                // Going to walk backwards and get all preceding whitespace until a newline or a character
                let item_diff_range = get_original_item(id)
                    .as_ref()
                    .unwrap()
                    .original()
                    .span()
                    .byte_range();
                let item_diff_start = item_diff_range.start;
                let item_diff_end = item_diff_range.end;

                let mut item_diff_whitespace_start = item_diff_start as isize - 1;
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

                if !source_code[item_diff_whitespace_start as usize].is_ascii_whitespace() {
                    // we hit a non-whitespace character which shouldn't be included in our output
                    item_diff_whitespace_start += 1;
                }

                let substring =
                    source_code[item_diff_whitespace_start as usize..item_diff_start].to_vec();
                let code = Code(String::from_utf8(substring).expect("Off a code boundary"));

                diffs.push((None, code));

                // then get the actual diff
                let viewable = id.as_viewable();
                for diff in viewable.vds {
                    if let Some(sub_diff) = get_sub_diffs(diff) {
                        diffs.extend(sub_diff);
                    }
                }

                i = item_diff_end;
            }
            None => {
                i += 1;
            }
        }
    }

    diffs
}

fn end_index_of_items(impl_: &ItemImpl, source_code: &[u8]) -> usize {
    let mut index_before_close_brace = impl_.span().byte_range().end - 2;
    while index_before_close_brace > 0
        && source_code[index_before_close_brace].is_ascii_whitespace()
    {
        index_before_close_brace -= 1;
    }

    index_before_close_brace += 1; // we want 1 index beyond the last char we ended at
    index_before_close_brace
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
