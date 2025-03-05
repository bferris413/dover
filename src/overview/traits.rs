use std::ops::Deref;

use syn::{ItemTrait, TraitItem};

use crate::Vis;

use super::generics::Generics;

#[derive(Debug)]
pub struct Traits(pub Vec<Trait>);
impl Traits {
    /// Creates a complete set of `struct` declarations from a list of `Trait`s.
    pub fn from(mut traits: Vec<Trait>) -> Self {
        traits.sort_by(|t1, t2| t1.name().cmp(&t2.name()));
        traits.dedup_by(|t1, t2| t1.name() == t2.name());
        Traits(traits)
    }
}
impl Deref for Traits {
    type Target = [Trait];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Trait {
    name: String,
    vis: Vis,
    generics: Generics,
    items: Vec<TraitItem>,
    original: ItemTrait,
}
impl Trait {
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl From<ItemTrait> for Trait {
    fn from(t: ItemTrait) -> Self {
        let original = t.clone();
        let name = t.ident.to_string();
        let vis = t.vis.into();
        let generics = Generics::from(t.generics);
        let items = t.items;
        Trait {
            name,
            vis,
            generics,
            items,
            original,
        }
    }
}
