#[test]
fn ui_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/correct.rs");
    t.compile_fail("tests/ui/not_function.rs");
    t.compile_fail("tests/ui/async.rs");
    t.compile_fail("tests/ui/attr_arguments.rs");
    t.compile_fail("tests/ui/fn_arguments.rs");
    t.compile_fail("tests/ui/no_return.rs");
    t.compile_fail("tests/ui/generics.rs");
    t.compile_fail("tests/ui/wrong_return.rs");
}
