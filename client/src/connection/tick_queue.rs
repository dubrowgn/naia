use naia_shared::Tick;
use std::{cmp::Ordering, collections::BinaryHeap};

/// A queue for items marked by tick, will only ever pop items from the queue if
/// the tick has elapsed
pub struct TickQueue<T> {
    queue: BinaryHeap<ItemContainer<T>>,
}

impl<T> TickQueue<T> {
    /// Create a new TimeQueue
    pub fn new() -> Self {
        TickQueue {
            queue: BinaryHeap::new(),
        }
    }

    /// Adds an item to the queue marked by tick
    pub fn add_item(&mut self, tick: Tick, item: T) {
        self.queue.push(ItemContainer { tick, item });
    }

    /// Returns whether or not there is an item that is ready to be returned
    fn has_item(&self, current_tick: Tick) -> bool {
        if self.queue.is_empty() {
            return false;
        }
        if let Some(item) = self.queue.peek() {
            return current_tick >= item.tick;
        }
        false
    }

    /// Pops an item from the queue if the tick has elapsed
    pub fn pop_item(&mut self, current_tick: Tick) -> Option<(Tick, T)> {
        if self.has_item(current_tick) {
            if let Some(container) = self.queue.pop() {
                return Some((container.tick, container.item));
            }
        }
        None
    }
}

pub struct ItemContainer<T> {
    pub tick: Tick,
    pub item: T,
}

impl<T> PartialEq for ItemContainer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.tick == other.tick
    }
}

impl<T> Eq for ItemContainer<T> {}

impl<T> Ord for ItemContainer<T> {
    fn cmp(&self, other: &ItemContainer<T>) -> Ordering {
		self.tick.cmp(&other.tick)
    }
}

impl<T> PartialOrd for ItemContainer<T> {
    fn partial_cmp(&self, other: &ItemContainer<T>) -> Option<Ordering> {
		self.tick.partial_cmp(&other.tick)
    }
}
