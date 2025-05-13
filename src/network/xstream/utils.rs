pub struct IdIterator {
    current: u128,
}

impl IdIterator {
    pub fn new() -> Self {
        Self { current: 0 }
    }
}

impl Iterator for IdIterator {
    type Item = u128;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        self.current = if current < u128::MAX { current + 1 } else { 0 };
        Some(current)
    }
}
