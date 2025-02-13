use syn::{GenericParam, Generics as SynGenerics, WhereClause};

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

        // extremely coarse, eventually we want to diff params themselves, but for
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
        if self == other {
            return None;
        }

        todo!()
    }
}

#[derive(Debug)]
pub struct GenericsDiff {
    params_diff: Option<Vec<GenericParamDiff>>,
    where_diff: Option<WhereClauseDiff>,
}

#[derive(Debug)]
pub struct GenericParamDiff {
    change: Change,
    param: Option<GenericParam>,
}
#[derive(Debug)]
pub struct WhereClauseDiff;
