use super::structs::Vis;
use syn::{FieldMutability, Type};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Field {
    name: Option<String>,
    vis: Vis,
    mutability: FieldMutability,
    ty: Type,
}
impl From<syn::Field> for Field {
    fn from(field: syn::Field) -> Self {
        let name = field.ident.map(|ident| ident.to_string());
        let vis = field.vis.into();
        let ty = field.ty;
        let mutability = field.mutability;

        Field {
            name,
            vis,
            ty,
            mutability,
        }
    }
}
