use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::fmt::Debug;

#[derive(Debug)]
struct Item<T: Ord + Debug> {
    inner: T,
    reverse: bool,
}

impl<T: Ord + Debug> Item<T> {
    fn new(inner: T, reverse: bool) -> Self {
        Self { inner, reverse }
    }
}

impl<T: Ord + Debug> PartialOrd for Item<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord + Debug> PartialEq for Item<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T: Ord + Debug> Eq for Item<T> {}

impl<T: Ord + Debug> Ord for Item<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        if !self.reverse {
            self.inner.cmp(&other.inner)
        } else {
            other.inner.cmp(&self.inner)
        }
    }
}

/// Merges sorted streams using a binary heap.
/// In case of ties, items are sorted by the order of the input streams, in which
/// they occur.
#[derive(Debug)]
pub struct HeapMerge<T, I, E>
where
    T: Ord + Debug,
    I: Iterator<Item = Result<T, E>>,
    E: Debug,
{
    streams: Box<[I]>,
    heap: BinaryHeap<Reverse<(Item<T>, usize)>>,
    reverse: bool,
}

impl<T, I, E> HeapMerge<T, I, E>
where
    T: Ord + Debug,
    I: Iterator<Item = Result<T, E>>,
    E: Debug,
{
    pub fn new<S>(streams: S, reverse: bool) -> Result<Self, E>
    where
        S: IntoIterator<Item = I>,
    {
        let mut streams = streams.into_iter().collect::<Box<[_]>>();
        let mut heap = BinaryHeap::with_capacity(streams.len());
        for (i, stream) in streams.iter_mut().enumerate() {
            if let Some(item) = stream.next() {
                heap.push(Reverse((Item::new(item?, reverse), i)));
            }
        }
        Ok(Self {
            heap,
            reverse,
            streams,
        })
    }
}

impl<T, I, E> Iterator for HeapMerge<T, I, E>
where
    T: Ord + Debug,
    I: Iterator<Item = Result<T, E>>,
    E: Debug,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        self.heap.pop().map(|top| {
            let (top_item, top_i) = top.0;
            if let Some(next_item) = self.streams[top_i].next() {
                self.heap
                    .push(Reverse((Item::new(next_item?, self.reverse), top_i)));
            }
            Ok(top_item.inner)
        })
    }
}
