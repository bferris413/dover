use syn::{GenericParam, Generics as SynGenerics, WhereClause};

use crate::Diff;

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

        // let mut diffs = Vec::new();
        // for (a, b) in self.iter().zip(other.iter()) {
        //     let diff = a.diff_with(b);
        //     diffs.push(diff);
        // }
        // Some(diffs)
        todo!()
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
pub struct GenericParamDiff;
#[derive(Debug)]
pub struct WhereClauseDiff;
