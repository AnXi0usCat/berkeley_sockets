use crate::kqueue::Kqueue;

#[derive(Debug)]
pub struct Reactor {
    kq: Kqueue,
}
