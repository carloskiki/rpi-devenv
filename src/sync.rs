use core::cell::UnsafeCell;

enum State<T, F> {
    Uninit(F),
    Init(T),
    Poisoned,
}

pub struct LazyCell<T, F = fn() -> T> {
    state: UnsafeCell<State<T, F>>,
}
