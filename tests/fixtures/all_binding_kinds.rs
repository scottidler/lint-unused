// Fixture: every binding kind that should be detected

fn function_with_param(_param: i32) {
    // fn param
}

fn main() {
    // let binding
    let _result = 42;

    // let mut binding
    let mut _counter = 0;
    _counter += 1;

    // closure param
    let _f = |_x: i32| _x + 1;

    // for loop
    for _item in [1, 2, 3] {}

    // match arm
    match Some(42) {
        Some(_val) => {}
        None => {}
    }

    // if let
    if let Some(_inner) = Some(42) {}

    // while let
    let mut iter = vec![1].into_iter();
    while let Some(_elem) = iter.next() {}

    // nested destructuring
    let (_a, (_b, _c)) = (1, (2, 3));
}
