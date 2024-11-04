use macros::main;

#[main]
pub fn kernel_main() -> ! {
    let a = 5;
    let b = 6;
    fn my_function(x: i32, y: i32) -> i32 {
        x + y
    }
    let c = my_function(a, b);
    todo!("{}", c)
}

fn main() {}
