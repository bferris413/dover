// use cake::cheese;

enum TrafficLight<T: Clone> {
    Red { t: T },
    Yellow,
    Green,
}

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
