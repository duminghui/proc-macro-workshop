// Write code here.
//
// To see what the code looks like after macro expansion:
//     $ cargo expand
//
// To run the code:
//     $ cargo run

#![allow(unused)]

// use derive_builder::Builder;
// #[derive(Builder)]
// pub struct Command {
//     executable:  String,
//     #[builder(each = "arg")]
//     args:        Vec<String>,
//     #[builder(each = "env")]
//     env:         Vec<String>,
//     current_dir: Option<String>,
// }

use std::fmt::Debug;

use derive_debug::CustomDebug;
pub trait Trait {
    type Value;
}

#[derive(CustomDebug)]
#[debug(bound = "T::Value: Debug")]
pub struct Wrapper<T: Trait> {
    field: Field<T>,
}

#[derive(CustomDebug)]
struct Field<T: Trait> {
    values: Vec<T::Value>,
}

fn assert_debug<F: Debug>() {
}

fn main() {
    struct Id;

    impl Trait for Id {
        type Value = u8;
    }

    assert_debug::<Wrapper<Id>>();
}
