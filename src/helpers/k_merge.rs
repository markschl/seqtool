use std::cmp::Ordering;
use std::fmt::Debug;
use std::{cmp::Reverse, collections::BinaryHeap};

#[derive(Debug)]
struct Item<T: Ord + Debug> {
    inner: T,
    reverse: bool,
    source: usize,
}

impl<T: Ord + Debug> Item<T> {
    fn new(inner: T, reverse: bool, source: usize) -> Self {
        Self {
            inner,
            reverse,
            source,
        }
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

#[derive(Debug)]
pub struct KMerge<T, I, E>
where
    T: Ord + Debug,
    I: Iterator<Item = Result<T, E>>,
    E: Debug,
{
    streams: Vec<I>,
    heap: BinaryHeap<Reverse<Item<T>>>,
    reverse: bool,
}

impl<T, I, E> KMerge<T, I, E>
where
    T: Ord + Debug,
    I: Iterator<Item = Result<T, E>>,
    E: Debug,
{
    pub fn new(mut streams: Vec<I>, reverse: bool) -> Result<Self, E> {
        let mut heap = BinaryHeap::with_capacity(streams.len());
        for (i, stream) in streams.iter_mut().enumerate() {
            if let Some(item) = stream.next() {
                heap.push(Reverse(Item::new(item?, reverse, i)));
            }
        }
        Ok(Self {
            heap,
            reverse,
            streams,
        })
    }
}

impl<T, I, E> Iterator for KMerge<T, I, E>
where
    T: Ord + Debug,
    I: Iterator<Item = Result<T, E>>,
    E: Debug,
{
    type Item = Result<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        self.heap.pop().map(|top| {
            if let Some(next_item) = self.streams[top.0.source].next() {
                self.heap
                    .push(Reverse(Item::new(next_item?, self.reverse, top.0.source)));
            }
            Ok(top.0.inner)
        })
    }
}
