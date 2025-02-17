use syn::{GenericParam, Generics as SynGenerics, WhereClause, WherePredicate};

use crate::{Change, Diff, ExistenceChange};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Generics {
    params: Vec<GenericParam>,
    where_clause: Option<WhereClause>,
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
        };

        Some(diff)
    }
}
impl From<SynGenerics> for Generics {
    fn from(generics: syn::Generics) -> Self {
        let params = generics.params.into_iter().collect();
        let where_clause = generics.where_clause;
        Self {
            params,
            where_clause,
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
                let change = Change::Existence(ExistenceChange::Deleted);
                let diff = GenericParamDiff {
                    change,
                    param: Some(old_param.clone()),
                };
                param_diffs.push(diff);
            }
        }

        for new_param in other.iter() {
            if !self.contains(new_param) {
                let change = Change::Existence(ExistenceChange::Added);
                let diff = GenericParamDiff {
                    change,
                    param: Some(new_param.clone()),
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
        match dbg!((self, other)) {
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
                        let change = Change::Existence(ExistenceChange::Deleted);
                        let diff = PredicateDiff {
                            change,
                            predicate: Some(predicate.clone()),
                        };
                        predicate_diffs.push(diff);
                    }
                }
                for predicate in w2.predicates.iter() {
                    if !w1.predicates.iter().find(|p1| *p1 == predicate).is_some() {
                        let change = Change::Existence(ExistenceChange::Added);
                        let diff = PredicateDiff {
                            change,
                            predicate: Some(predicate.clone()),
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
#[allow(dead_code)]
pub struct PredicateDiff {
    change: Change,
    predicate: Option<WherePredicate>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct GenericsDiff {
    params_diff: Option<Vec<GenericParamDiff>>,
    where_diff: Option<WhereClauseDiff>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct GenericParamDiff {
    change: Change,
    param: Option<GenericParam>,
}
#[derive(Debug)]
#[allow(dead_code)]
pub struct WhereClauseDiff {
    change: Change,
    where_clause: Option<WhereClause>,
    predicates: Option<Vec<PredicateDiff>>,
}
