// 这个文件为了说明
// dyn 和 generic static dispatch相冲突
// generic static dispatch会被编译成不同的 extend_int 函数，最后生成的代码并不会有 extend 函数
// 而 dyn 多态必须具备 add 函数
// 所以两者相互冲突

// use std::vec;

// pub trait Hei {
//     fn hei(&self);
// }

// impl Hei for i32 {
//     fn hei(&self) {
//         println!("{}", &self);
//     }
// }

// pub trait GenericHei<T> {
//     fn extend<I>(&self, a:T);
// }
// struct MyV<T>(Vec<T>);

// impl<T> GenericHei<T> for MyV<T> {
//     fn extend<I>(&self, a:I) {
        
//     }
// }
// pub fn add_true<T>(a: &dyn GenericHei<T>, x: T) {
//     a.extend(x);
// }
// fn main() {
//     let a :i32 = 2;
//     a.hei();
//     let a = add_true(&MyV {
//         0: vec![1],
//     },1);
// }

fn main(){}