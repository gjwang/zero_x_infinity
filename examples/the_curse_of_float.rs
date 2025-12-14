fn main() {
    let a: f64 = 0.1;
    let b: f64 = 0.2;
    let sum = a + b;

    // You expect this to pass, right?
    if sum == 0.3 {
        println!("Math works!");
    } else {
        println!("PANIC: Math is broken! Sum is {:.20}", sum);
    }
}
