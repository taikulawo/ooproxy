use std::cell::{Cell, RefCell};
#[derive(Debug)]
pub struct Myself<'a> {
    name: & 'static str,
    location: RefCell<String>,
    age: Cell<u32>,
    a: &'a mut u32,
}
fn main() {
    let my = Myself {
        age: Cell::new(22),
        location: RefCell::new(String::from("beijing")),
        name: "wuweichao",
        a: & mut 1
    };
    my.age.set(1);
    my.location.borrow();
    my.location.borrow();
    my.location.borrow();

    *(my.a) = 2;
    let a1 = &my.a;
    let a2 = &my.a;

    println!("{:?}", &my);
}