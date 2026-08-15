use std::io::{Read, Seek, Write};
use tokio::net::TcpStream;

async fn build<T, U, V>(x: String, y: T) -> u32
where
    U: Read,
    V: Write + Send,
{
    todo!()
}

enum ApplicationProtocol<T: Clone + Send, U> {
    Dns,
    Http { t: T, u: U },
    Https { t: T },
}

struct RandomStruct {
    field1: u8,
    field3: u32,
}
impl RandomStruct {
    fn new(field1: u8, field3: u32) -> Self {
        Self { field1, field3 }
    }
    pub fn do_it(&self, x: String, y: String) {
        println!("do_it called with ({x}, {y})");
    }
    pub fn do_it_again(&self) -> Result<(), String> {
        println!("I'm doing it again!");
        Ok(())
    }
}

// const unsafe fn run(
//     more: StringsWithNumbersAndStuff,
//     i: u32,
//     b: T,
//     c: A,
//     f: AVeryLongTypeNameThatsAnnoying,
// ) -> std::u32::u32 {
//     i
// }

// fn cake() {}
// fn doit() {
//     println!("yes");
// }
//

// const unsafe fn run(i: u32) -> std::u32::u32 {
//     i
// }

// struct Struct2;
// impl Struct {
//     fn new(field1: u32, field2: String) -> Self {
//         Self { field1, field2 }
//     }

//     fn milk(&self) -> Rice {
//         println!("run");
//     }
// }

// impl SanityCheck {
//     pub fn sanity_check(&self) -> DoThing {
//         println!("hello");
//     }
// }

// impl View for SanityCheck {
//     fn run(&self) -> Rice {
//         Rice::new(eggs)
//     }
// }

// trait Milkshake {
//     fn cheese<T>(&mut self) -> eggs::Cheese;
// }

// trait Rice {}

// trait Rice {}

// enum Person {
//     Idea,
// }

// enum Buddy {
//     Friend,
//     Pal,
//     Human,
// }

// pub struct Struct1(String);
// struct Struct3;

// struct Struct {
//     field1: u32,
//     field2: String,
// }

// struct Thing<T, U>(T, U)
// where
//     U: Cake,
//     T: Eggs;

// enum EnumAdded {
//     Variant1,
//     Variant2,
//     Variant3,
// }

// enum VariantDiff {
//     Variant2,
//     Variant1 { field1: u32, field2: String },
// }
// // struct FieldAddedTuple(u8, u16, u32);

// struct FieldAddedNormal {
//     a1: String,
//     f1: u16,
//     f2: u32,
//     f3: u64,
// }

// struct UnitToTuple(u8, u16);
// struct TupleToUnit;
// struct UnitToFields {
//     f1: String,
//     f2: u32,
// }
// struct FieldsToUnit;

// struct FieldsToTuple(String, u32);

// struct TupleToFields {
//     field1: String,
//     field2: u32,
// }

// struct FieldsToFields {
//     field1: String,
//     field2: u32,
// }
