use std::{cell::{Cell, RefCell}, ops::Deref};
#[derive(Debug)]
pub struct Myself<'a> {
    name: &'static str,
    location: RefCell<String>,
    age: Cell<u32>,
    a: &'a mut u32,
}

fn print(s: &mut String){
    let a = &mut *s;
    take(a);
    &mut *s;
}
fn take(s: &String){}

struct MyBox<T>(T);
impl<T> MyBox<T> {
    fn new(v: T) -> MyBox<T> {
        MyBox(v)
    }
}

impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
fn hello(s: &str) {}
fn main() {
    let mut s = String::from("");
    let a = MyBox::new(s);
    hello(&*a);
}
