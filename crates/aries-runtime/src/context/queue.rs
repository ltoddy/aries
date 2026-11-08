use std::collections::VecDeque;

pub struct BoundedQueue<T> {
    items: VecDeque<T>,
    max_size: usize,
}

impl<T> BoundedQueue<T> {
    pub fn new(max_size: usize) -> Self {
        Self { items: VecDeque::with_capacity(max_size), max_size }
    }

    pub fn push(&mut self, item: T) -> Option<T> {
        let old = if self.items.len() == self.max_size { self.items.pop_front() } else { None };
        self.items.push_back(item);
        old
    }
}
