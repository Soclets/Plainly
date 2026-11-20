use plainly::Call;

#[plainly::procedure("pick")]
fn pick(call: &mut Call, index: i64) -> i64 {
    let _ = call;
    let numbers = [10, 20, 30];
    numbers[index as usize]
}
