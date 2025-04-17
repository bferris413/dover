use std::ops::Range;

use syn::{spanned::Spanned, GenericParam, Generics as SynGenerics, WhereClause, WherePredicate};

use crate::{ByteRange, Change, Diff, ExistenceChange};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Generics {
    params: Vec<GenericParam>,
    where_clause: Option<WhereClause>,
    original: SynGenerics,
}
impl Diff for Generics {
    type Diff = Option<GenericsDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let params_diff = self.params.diff_with(&other.params);
        let where_diff = self.where_clause.diff_with(&other.where_clause);

        // it's possible for generics to be non-equal and yet the diffs don't contain
        // anything we're interested in tracking.
        if params_diff.is_none() && where_diff.is_none() {
            return None;
        }

        let diff = GenericsDiff {
            params_diff,
            where_diff,
            old: self.original.clone(),
            new: other.original.clone(),
        };

        Some(diff)
    }
}
impl From<SynGenerics> for Generics {
    fn from(generics: syn::Generics) -> Self {
        let params = generics.params.iter().map(|p| p.clone()).collect();
        let where_clause = generics.where_clause.clone();
        Self {
            params,
            where_clause,
            original: generics,
        }
    }
}

impl Diff for Vec<GenericParam> {
    type Diff = Option<Vec<GenericParamDiff>>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        if self == other {
            return None;
        }

        let mut param_diffs = Vec::new();

        // extremely coarse. eventually we want to diff params themselves, but for
        // now we just use full equality with added/removed changes (no modifications)
        for old_param in self.iter() {
            if !other.contains(old_param) {
                let change = ExistenceChange::Deleted;
                let diff = GenericParamDiff {
                    change,
                    param: old_param.clone(),
                };
                param_diffs.push(diff);
            }
        }

        for new_param in other.iter() {
            if !self.contains(new_param) {
                let change = ExistenceChange::Added;
                let diff = GenericParamDiff {
                    change,
                    param: new_param.clone(),
                };
                param_diffs.push(diff);
            }
        }

        if param_diffs.is_empty() {
            return None;
        } else {
            Some(param_diffs)
        }
    }
}

impl Diff for Option<WhereClause> {
    type Diff = Option<WhereClauseDiff>;
    fn diff_with(&self, other: &Self) -> Self::Diff {
        match (self, other) {
            (None, None) => None,
            (Some(w1), Some(w2)) if w1 == w2 => None,
            (Some(w1), None) => {
                let change = Change::Existence(ExistenceChange::Deleted);
                let diff = WhereClauseDiff {
                    change,
                    where_clause: Some(w1.clone()),
                    predicates: None,
                };
                Some(diff)
            }
            (None, Some(w2)) => {
                let change = Change::Existence(ExistenceChange::Added);
                let diff = WhereClauseDiff {
                    change,
                    where_clause: Some(w2.clone()),
                    predicates: None,
                };
                Some(diff)
            }
            (Some(w1), Some(w2)) => {
                // coarse-grained predicate diff (only supports existence changes)
                let mut predicate_diffs = Vec::new();
                for predicate in w1.predicates.iter() {
                    if !w2.predicates.iter().find(|p2| *p2 == predicate).is_some() {
                        let change = ExistenceChange::Deleted;
                        let diff = PredicateDiff {
                            change,
                            predicate: predicate.clone(),
                        };
                        predicate_diffs.push(diff);
                    }
                }
                for predicate in w2.predicates.iter() {
                    if !w1.predicates.iter().find(|p1| *p1 == predicate).is_some() {
                        let change = ExistenceChange::Added;
                        let diff = PredicateDiff {
                            change,
                            predicate: predicate.clone(),
                        };
                        predicate_diffs.push(diff);
                    }
                }

                assert!(!predicate_diffs.is_empty());
                Some(WhereClauseDiff {
                    change: Change::Modified,
                    where_clause: None,
                    predicates: Some(predicate_diffs),
                })
            }
        }
    }
}

#[derive(Debug)]
pub struct PredicateDiff {
    change: ExistenceChange,
    predicate: WherePredicate,
}
impl ByteRange for PredicateDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let ExistenceChange::Deleted = self.change else {
            return Vec::new();
        };

        let old_range = self.predicate.span().byte_range();
        vec![old_range]
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let ExistenceChange::Added = self.change else {
            return Vec::new();
        };

        let new_range = self.predicate.span().byte_range();
        vec![new_range]
    }
}
impl PredicateDiff {
    pub fn change(&self) -> ExistenceChange {
        self.change
    }
    pub fn predicate(&self) -> &WherePredicate {
        &self.predicate
    }
}

#[derive(Debug)]
pub struct GenericsDiff {
    params_diff: Option<Vec<GenericParamDiff>>,
    where_diff: Option<WhereClauseDiff>,
    old: SynGenerics,
    new: SynGenerics,
}
impl ByteRange for GenericsDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        let mut old_ranges = Vec::new();

        if let Some(params_diff) = &self.params_diff {
            params_diff
                .iter()
                .for_each(|gpd| old_ranges.append(&mut gpd.old_ranges()));

            old_ranges.push(self.old.span().byte_range());
        };

        if let Some(where_diff) = &self.where_diff() {
            let mut where_ranges = where_diff.old_ranges();
            old_ranges.append(&mut where_ranges);
        }

        old_ranges
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        let mut new_ranges = Vec::new();

        if let Some(params_diff) = &self.params_diff {
            params_diff
                .iter()
                .for_each(|gpd| new_ranges.append(&mut gpd.new_ranges()));

            new_ranges.push(self.new.span().byte_range());
        };

        if let Some(where_diff) = &self.where_diff() {
            let mut where_ranges = where_diff.new_ranges();
            new_ranges.append(&mut where_ranges);
        }

        new_ranges
    }
}
impl GenericsDiff {
    pub fn params_diff(&self) -> Option<&Vec<GenericParamDiff>> {
        self.params_diff.as_ref()
    }
    pub fn where_diff(&self) -> Option<&WhereClauseDiff> {
        self.where_diff.as_ref()
    }
}

#[derive(Debug)]
pub struct GenericParamDiff {
    change: ExistenceChange,
    param: GenericParam,
}
impl ByteRange for GenericParamDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        match self.change {
            ExistenceChange::Added => Vec::new(),
            ExistenceChange::Deleted => {
                let old_range = self.param.span().byte_range();
                if !old_range.is_empty() {
                    vec![old_range]
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        match self.change {
            ExistenceChange::Added => {
                let new_range = self.param.span().byte_range();
                if !new_range.is_empty() {
                    vec![new_range]
                } else {
                    Vec::new()
                }
            }
            ExistenceChange::Deleted => Vec::new(),
        }
    }
}
impl GenericParamDiff {
    pub fn change(&self) -> ExistenceChange {
        self.change
    }
    pub fn param(&self) -> &GenericParam {
        &self.param
    }
}

#[derive(Debug)]
pub struct WhereClauseDiff {
    change: Change,
    where_clause: Option<WhereClause>,
    predicates: Option<Vec<PredicateDiff>>,
}
impl ByteRange for WhereClauseDiff {
    fn old_ranges(&self) -> Vec<Range<usize>> {
        match self.change {
            Change::Existence(ex) => {
                // this isn't expressed cleanly in the struct yet, just needs design
                assert!(self.where_clause.is_some());
                let wc = &self.where_clause.as_ref().unwrap();
                match ex {
                    ExistenceChange::Added => Vec::new(),
                    ExistenceChange::Deleted => vec![wc.span().byte_range()],
                }
            }
            Change::Modified => {
                assert!(self.predicates.is_some());
                let predicates = self.predicates.as_ref().unwrap();
                let mut old_ranges = Vec::new();

                predicates
                    .iter()
                    .for_each(|pd| old_ranges.append(&mut pd.old_ranges()));

                old_ranges
            }
        }
    }

    fn new_ranges(&self) -> Vec<Range<usize>> {
        match self.change {
            Change::Existence(ex) => {
                // this isn't expressed cleanly in the struct yet, just needs design
                assert!(self.where_clause.is_some());
                let wc = &self.where_clause.as_ref().unwrap();
                match ex {
                    ExistenceChange::Added => Vec::new(),
                    ExistenceChange::Deleted => vec![wc.span().byte_range()],
                }
            }
            Change::Modified => {
                assert!(self.predicates.is_some());
                let predicates = self.predicates.as_ref().unwrap();
                let mut new_ranges = Vec::new();

                predicates
                    .iter()
                    .for_each(|pd| new_ranges.append(&mut pd.new_ranges()));

                new_ranges
            }
        }
    }
}

impl WhereClauseDiff {
    pub fn change(&self) -> Change {
        self.change
    }
    pub fn where_clause(&self) -> Option<&WhereClause> {
        self.where_clause.as_ref()
    }
    pub fn predicates(&self) -> Option<&Vec<PredicateDiff>> {
        self.predicates.as_ref()
    }
}
