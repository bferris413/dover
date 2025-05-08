use cake::cheese::eggs;
use rice;

// const unsafe fn run(
//     more: StringsWithNumbersAndStuff,
//     i: u32,
//     b: T,
//     c: A,
//     f: AVeryLongTypeNameThatsAnnoying,
// ) -> std::u32::u32 {
//     i
// }

fn build(x: String) -> u32 {
    todo!()
}

fn cake() {}
// fn doit() {
//     println!("yes");
// }
//
// pub struct Struct1(String);

// const unsafe fn run(i: u32) -> std::u32::u32 {
//     i
// }

struct Struct2;
struct Struct {
    field1: u32,
    field2: String,
}
impl Struct {
    fn new(field1: u32, field2: String) -> Self {
        Self { field1, field2 }
    }
}
//struct Struct3;

trait Milkshake {
    fn cheese<T>(&mut self) -> eggs::Cheese;
}

trait Rice {}

// trait Rice {}

enum L7Protocol {
    Https { t: T },
    Dns,
    Smtp,
}

enum Person {
    Idea,
}

// struct Thing<T, U>(T, U)
// where
//     U: Cake,
//     T: Eggs;
