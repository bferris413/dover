use std::fmt::{Display, Formatter};
use std::ops::Range;

use syn::{
    spanned::Spanned,
    token::{Async, Const, Unsafe},
    Abi, FnArg, ImplItemFn, ItemFn, ReturnType, Visibility,
};
use syn::{Block, TraitItemFn};

use super::generics::{Generics, GenericsDiff};
use crate::{collect_src_maps, ByteRange, Code, ViewableDiffs};
use crate::{Change, Diff, ExistenceChange, SourceFile, View, ViewableDiff, VisDiff};

const NO_SRC_ERROR: &str = "No source text for function, was parse logic changed?";

/// A collection of freestanding `fn` definitions.
///
/// The internal representation is sorted and deduped.
#[derive(Debug, Eq, PartialEq)]
pub struct Functions(Vec<Function>);
impl Functions {
    /// Creates a complete set of freestanding `Function` declarations from a list of `syn::ItemFn`.
    pub fn new_freestanding(fns: Vec<ItemFn>, source: SourceFile) -> Self {
        let mut functions: Vec<Function> = fns
            .into_iter()
            .map(|item| Function::new_freestanding(item, source.clone()))
            .collect();
        functions.sort_by(|f1, f2| f1.name().cmp(&f2.name()));
        functions.dedup_by(|f1, f2| f1.name() == f2.name());
        Functions(functions)
    }
    pub fn new_impl(fns: Vec<ImplItemFn>, source: SourceFile) -> Self {
        let mut functions: Vec<Function> = fns
            .into_iter()
            .map(|item| Function::new_impl(item, source.clone()))
            .collect();
        functions.sort_by(|f1, f2| f1.name().cmp(&f2.name()));
        functions.dedup_by(|f1, f2| f1.name() == f2.name());
        Functions(functions)
    }
    pub fn new_trait(fns: Vec<TraitItemFn>, source: SourceFile) -> Self {
        let mut functions: Vec<Function> = fns
            .into_iter()
            .map(|item| Function::new_trait(item, source.clone()))
            .collect();
        functions.sort_by(|f1, f2| f1.name().cmp(&f2.name()));
        functions.dedup_by(|f1, f2| f1.name() == f2.name());
        Functions(functions)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn functions(&self) -> &[Function] {
        &self.0
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
    vis: Visibility,
    r#const: Option<Const>,
    r#async: Option<Async>,
    r#unsafe: Option<Unsafe>,
    abi: Option<Abi>,
    name: String,
    generics: Generics,
    inputs: Inputs,
    output: ReturnType,
    original_fn: ItemFn,
    source: SourceFile,
}
impl Function {
    pub fn new_freestanding(f: ItemFn, source: SourceFile) -> Self {
        Function {
            vis: f.vis.clone(),
            r#const: f.sig.constness,
            r#async: f.sig.asyncness,
            r#unsafe: f.sig.unsafety,
            abi: f.sig.abi.clone(),
            name: f.sig.ident.to_string(),
            generics: Generics::from(f.sig.generics.clone()),
            inputs: Inputs {
                args: f.sig.clone().inputs.into_iter().collect(),
            },
            output: f.sig.output.clone(),
            original_fn: f.clone(),
            source,
        }
    }
    pub fn new_impl(f: ImplItemFn, source: SourceFile) -> Self {
        Function {
            vis: f.vis.clone(),
            r#const: f.sig.constness,
            r#async: f.sig.asyncness,
            r#unsafe: f.sig.unsafety,
            abi: f.sig.abi.clone(),
            name: f.sig.ident.to_string(),
            generics: Generics::from(f.sig.generics.clone()),
            inputs: Inputs {
                args: f.sig.clone().inputs.into_iter().collect(),
            },
            output: f.sig.output.clone(),
            // TODO: bad and tricky, plus we're losing a piece of ImplItemFn ('default' field)
            original_fn: ItemFn {
                attrs: f.attrs,
                vis: f.vis,
                sig: f.sig,
                block: Box::new(f.block),
            },
            source,
        }
    }
    pub fn new_trait(f: TraitItemFn, source: SourceFile) -> Self {
        let vis = Visibility::Inherited;
        let empty_block: Block = syn::parse_str("{}").unwrap();
        Function {
            vis: vis.clone(),
            r#const: f.sig.constness,
            r#async: f.sig.asyncness,
            r#unsafe: f.sig.unsafety,
            abi: f.sig.abi.clone(),
            name: f.sig.ident.to_string(),
            generics: Generics::from(f.sig.generics.clone()),
            inputs: Inputs {
                args: f.sig.clone().inputs.into_iter().collect(),
            },
            output: f.sig.output.clone(),
            // TODO: bad and tricky, plus we're losing a default field
            original_fn: ItemFn {
                attrs: f.attrs,
                vis,
                sig: f.sig,
                block: Box::new(empty_block),
            },
            source,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn original(&self) -> &ItemFn {
        &self.original_fn
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let source = remove_block(self.original_fn.span().source_text().expect(NO_SRC_ERROR));
        write!(f, "{source}")
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

        // take all the diffs, if old/new exists, get byte range and store in vec
        let (old_src_map, new_src_map) = collect_src_maps!(
            vis_diff,
            const_diff,
            async_diff,
            unsafe_diff,
            abi_diff,
            generics_diff,
            inputs_diff,
            return_type_diff,
        );

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
            old_src_map,
            new_src_map,
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
    old_src_map: Vec<Range<usize>>,
    new_src_map: Vec<Range<usize>>,
}
impl FunctionDiff {
    pub(crate) fn old(&self) -> &Option<Function> {
        &self.old
    }
    pub(crate) fn new(&self) -> &Option<Function> {
        &self.new
    }
}

impl View for FunctionDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        if let Change::Existence(_ex) = self.change {
            match (&self.old, &self.new) {
                (Some(_), Some(_)) => panic!("old and new functions were both Some"),
                (None, None) => panic!("old and new functions were both None"),
                (Some(old_func), None) => {
                    return ViewableDiffs::new(vec![ViewableDiff {
                        old: Some(vec![(
                            Some(ExistenceChange::Deleted),
                            Code(format!("{old_func}\n")),
                        )]),
                        new: None,
                    }]);
                }
                (None, Some(new_func)) => {
                    return ViewableDiffs::new(vec![ViewableDiff {
                        old: None,
                        new: Some(vec![(
                            Some(ExistenceChange::Added),
                            Code(format!("{new_func}\n")),
                        )]),
                    }]);
                }
            };
        }

        let old = self.old.as_ref().unwrap();
        let old_src = &old.source.0.as_bytes();
        let old_start = old.original_fn.span().byte_range().start;
        let old_end = old.original_fn.block.span().byte_range().start;
        let old_range = old_start..old_end;

        let mut i = old_range.start;
        let mut src_i = 0;
        let mut old_diff = Vec::new();
        // dbg!(&self.old_src_map);

        while i < old_range.end {
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
                    while i < old_range.end {
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
                    old_diff.push((None, code));
                }
            }
        }

        let new = self.new.as_ref().unwrap();
        let new_src = &new.source.0.as_bytes();
        let new_start = new.original_fn.span().byte_range().start;
        let new_end = new.original_fn.block.span().byte_range().start;
        let new_range = new_start..new_end;

        let mut i = new_range.start;
        let mut src_i = 0;
        let mut new_diff = Vec::new();

        while i < new_range.end {
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
                    while i < new_range.end {
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
                    new_diff.push((None, code));
                }
            }
        }

        ViewableDiffs::new(vec![ViewableDiff {
            old: Some(old_diff),
            new: Some(new_diff),
        }])
    }
}
impl ByteRange for FunctionDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let mut old_ranges = Vec::new();

        if let Some(ref vis_diff) = self.vis_diff {
            old_ranges.append(&mut vis_diff.old_ranges());
        }
        if let Some(ref const_diff) = self.const_diff {
            old_ranges.append(&mut const_diff.old_ranges());
        }
        if let Some(ref async_diff) = self.async_diff {
            old_ranges.append(&mut async_diff.old_ranges());
        }
        if let Some(ref unsafe_diff) = self.unsafe_diff {
            old_ranges.append(&mut unsafe_diff.old_ranges());
        }
        if let Some(ref abi_diff) = self.abi_diff {
            old_ranges.append(&mut abi_diff.old_ranges());
        }
        if let Some(ref generics_diff) = self.generics_diff {
            old_ranges.append(&mut generics_diff.old_ranges());
        }
        if let Some(ref inputs_diff) = self.inputs_diff {
            old_ranges.append(&mut inputs_diff.old_ranges());
        }
        if let Some(ref return_type_diff) = self.return_type_diff {
            old_ranges.append(&mut return_type_diff.old_ranges());
        }

        old_ranges
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let mut new_ranges = Vec::new();

        if let Some(ref abi_diff) = self.abi_diff {
            new_ranges.append(&mut abi_diff.new_ranges());
        }
        if let Some(ref vis_diff) = self.vis_diff {
            new_ranges.append(&mut vis_diff.new_ranges());
        }
        if let Some(ref const_diff) = self.const_diff {
            new_ranges.append(&mut const_diff.new_ranges());
        }
        if let Some(ref async_diff) = self.async_diff {
            new_ranges.append(&mut async_diff.new_ranges());
        }
        if let Some(ref unsafe_diff) = self.unsafe_diff {
            new_ranges.append(&mut unsafe_diff.new_ranges());
        }
        if let Some(ref generics_diff) = self.generics_diff {
            new_ranges.append(&mut generics_diff.new_ranges());
        }
        if let Some(ref inputs_diff) = self.inputs_diff {
            new_ranges.append(&mut inputs_diff.new_ranges());
        }
        if let Some(ref return_type_diff) = self.return_type_diff {
            new_ranges.append(&mut return_type_diff.new_ranges());
        }

        new_ranges
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
impl ByteRange for FnArgDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        if let Some(old) = &self.old {
            let byte_range = old.span().byte_range();
            vec![byte_range]
        } else {
            Vec::new()
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        if let Some(new) = &self.new {
            let byte_range = new.span().byte_range();
            vec![byte_range]
        } else {
            Vec::new()
        }
    }
}
#[allow(unused)]
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
impl View for FunctionsDiff {
    fn as_viewable(&self) -> ViewableDiffs {
        let ex_diffs = self
            .diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Existence(_)));

        let mut viewables = ViewableDiffs::empty();
        for diff in ex_diffs {
            viewables.append(diff.as_viewable());
        }

        // add/delete diffs should be side-by-side
        viewables.collapse();

        let mod_diffs = self
            .diffs
            .iter()
            .filter(|diff| matches!(diff.change, Change::Modified));

        for diff in mod_diffs {
            viewables.append(diff.as_viewable());
        }

        viewables
    }
}
impl ByteRange for FunctionsDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        self.diffs
            .iter()
            .map(|diff| diff.old_ranges())
            .flatten()
            .collect()
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        self.diffs
            .iter()
            .map(|diff| diff.new_ranges())
            .flatten()
            .collect()
    }
}
impl FunctionsDiff {
    pub(crate) fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }
    pub(crate) fn diffs(&self) -> &[FunctionDiff] {
        &self.diffs
    }
}

#[derive(Debug)]
pub struct InputsDiff {
    inputs: Vec<Option<FnArgDiff>>,
}
impl ByteRange for InputsDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let mut old_ranges = Vec::new();
        for input in self.inputs.iter() {
            if let Some(arg_diff) = input {
                old_ranges.append(&mut arg_diff.old_ranges());
            }
        }
        old_ranges
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let mut new_ranges = Vec::new();
        for input in self.inputs.iter() {
            if let Some(arg_diff) = input {
                new_ranges.append(&mut arg_diff.new_ranges());
            }
        }
        new_ranges
    }
}
#[allow(unused)]
impl InputsDiff {
    fn diffs(&self) -> &[Option<FnArgDiff>] {
        &self.inputs
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstDiff {
    existence: ExistenceChange,
    old: Option<Const>,
    new: Option<Const>,
}
impl ByteRange for ConstDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        if let Some(old) = &self.old {
            let range = old.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        if let Some(new) = &self.new {
            let range = new.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsyncDiff {
    existence: ExistenceChange,
    old: Option<Async>,
    new: Option<Async>,
}
impl ByteRange for AsyncDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        if let Some(old) = &self.old {
            let range = old.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        if let Some(new) = &self.new {
            let range = new.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsafeDiff {
    pub existence: ExistenceChange,
    pub old: Option<Unsafe>,
    pub new: Option<Unsafe>,
}
impl ByteRange for UnsafeDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        if let Some(old) = &self.old {
            let range = old.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        if let Some(new) = &self.new {
            let range = new.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiDiff {
    change: Change,
    old: Option<Abi>,
    new: Option<Abi>,
}
impl ByteRange for AbiDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        if let Some(old) = &self.old {
            let range = old.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        if let Some(new) = &self.new {
            let range = new.span().byte_range();
            vec![range]
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug)]
pub struct ReturnTypeDiff {
    old: ReturnType,
    new: ReturnType,
}
impl ByteRange for ReturnTypeDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let range = self.old.span().byte_range();
        vec![range]
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let range = self.new.span().byte_range();
        vec![range]
    }
}

#[allow(unused)]
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
            (None, Some(unsfe)) => Some(UnsafeDiff {
                existence: ExistenceChange::Added,
                old: None,
                new: Some(unsfe.clone()),
            }),
            (Some(unsfe), None) => Some(UnsafeDiff {
                existence: ExistenceChange::Deleted,
                old: Some(unsfe.clone()),
                new: None,
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
            (None, Some(c)) => Some(AsyncDiff {
                existence: ExistenceChange::Added,
                old: None,
                new: Some(c.clone()),
            }),
            (Some(c), None) => Some(AsyncDiff {
                existence: ExistenceChange::Deleted,
                old: Some(c.clone()),
                new: None,
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
            (None, Some(c)) => Some(ConstDiff {
                existence: ExistenceChange::Added,
                old: None,
                new: Some(c.clone()),
            }),
            (Some(c), None) => Some(ConstDiff {
                existence: ExistenceChange::Deleted,
                old: Some(c.clone()),
                new: None,
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
                old: Some(old.clone()),
                new: Some(new.clone()),
            }),
            (None, Some(new)) => Some(AbiDiff {
                change: Change::Existence(ExistenceChange::Added),
                old: None,
                new: Some(new.clone()),
            }),
            (Some(old), None) => Some(AbiDiff {
                change: Change::Existence(ExistenceChange::Deleted),
                old: Some(old.clone()),
                new: None,
            }),
            (None, None) => None,
        }
    }
}
