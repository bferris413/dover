use std::io::{Read, Write};
use tokio::net::TcpListener;

enum ApplicationProtocol<T: Clone> {
    Http { t: T },
    Https,
    Stun,
    Smtp,
}

pub fn build<T, U, V>(i: u32, y: T) -> T
where
    U: Read,
    V: Write,
{
    i
}

struct RandomStruct {
    field1: u8,
    field2: u16,
    field3: u32,
}
impl RandomStruct {
    fn new(field1: u8, field2: u16, field3: u32) -> Self {
        Self {
            field1,
            field2,
            field3,
        }
    }
    fn do_it(&self, x: String) {
        println!("do_it called with x: {}", x);
    }
}

// pub fn doit() {
//     println!("yes");
// }

// pub fn cake() {}

// fn eggs() {}

// trait Milkshake<T: Clone> {
//     fn cheese(&self) -> cheese::Cheese;
// }

// trait Cake {}
// impl SanityCheck {
//     pub fn rice() -> () {
//         ()
//     }
// }

// enum Buddy {
//     Friend,
//     Pal,
//     Person,
// }

// impl Skunk for Rice {
//     fn cake() -> () {}
// }
//

// fn escape_test() -> Result<div> {
//     // <div>this 'is & " some text</div>
// }

// enum EnumAdded {
//     Variant1,
//     Variant2,
// }

// enum VariantDiff {
//     Variant1 { field2: String },
//     Variant2,
// }

// // struct Struct1;
// // struct FieldAddedTuple(u8, u16);
// struct FieldAddedNormal {
//     f1: u16,
//     f2: u32,
// }
// impl Struct {
//     fn new(field2: u64) -> Self {
//         Self
//     }
// }

// pub struct Thing<T, U, V: Clone, W, X>
// where
//     X: Cake,
//     U: Powder,
//     W: Rice,
//     Y: Milk,
// {
//     field1: T,
//     field2: U,
//     field3: V,
// }

// struct UnitToTuple;
// struct TupleToUnit(String);
// struct UnitToFields;
// struct FieldsToUnit {
//     field1: String,
//     field2: u32,
// }

// struct FieldsToTuple {
//     field1: String,
//     field2: u32,
// }

// struct TupleToFields(String, u32);
