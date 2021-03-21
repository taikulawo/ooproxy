use std:: {
    borrow::Cow
};

fn main() {
    let x = "123";
    let a  = &x.to_owned();
    let a = String::from("123");
    let a1 = (&a).clone();
    let a2 = a.to_owned();
    assert!(a1 == a2);
}