use std::{
    collections::HashMap,
    cmp,
};

type BlockId = usize;

const BLOCK_SIZE: usize = 16 * 1024;
const BLOCK_MIN_NLINES: usize = 1024;
const DEFAULT_CACHE_CAPACITY: usize = 64;

#[derive(Debug)]
pub struct Block {
    id: BlockId,
    data: [u8; BLOCK_SIZE],
    size: usize,
    nlines: usize,

    prev: Option<BlockId>,
    next: Option<BlockId>,
}

impl Block {
    fn new(id: BlockId) -> Block {
        Block {
            id,
            data: [0; BLOCK_SIZE],
            size: 0,
            nlines: 0,
            prev: None,
            next: None,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..self.size]
    }

    pub fn offset(&self) -> usize {
        self.id * BLOCK_SIZE
    }

    /// Returns the number of lines, including a final line without a trailing '\n'.
    pub fn nlines(&self) -> usize {
        self.nlines
    }

    /// Appends bytes to the block.
    ///
    /// Returns any bytes that could not be appended if the block becomes full.
    fn append<'a>(&mut self, bytes: &'a [u8]) -> Option<&'a [u8]> {
        if self.size > 0 && self.data[self.size - 1] == b'\n' {
            self.nlines -= 1;
        }

        let remaining = BLOCK_SIZE - self.size;
        let bytes_to_append = cmp::min(remaining, bytes.len());
        for c in &bytes[..bytes_to_append] {
            if *c == b'\n' {
                self.nlines += 1;
            }
            self.data[self.size] = *c;
            self.size += 1;
        }

        self.nlines += 1;
        (remaining < bytes.len()).then_some(&bytes[bytes_to_append..])
    }
}

const fn offset_to_id(offset: usize) -> BlockId {
    offset / BLOCK_SIZE
}

/// LRU block cache
#[derive(Debug)]
pub struct BlockCache {
    blocks: HashMap<BlockId, Block>,
    head: Option<BlockId>,              // MRU
    tail: Option<BlockId>,              // LRU
    capacity: Option<usize>,
}

impl BlockCache {
    pub fn new() -> Self {
        Self::with_capacity(Some(DEFAULT_CACHE_CAPACITY))
    }

    pub fn with_capacity(capacity: Option<usize>) -> Self {
        Self {
            blocks: HashMap::new(),
            head: None,
            tail: None,
            capacity,
        }
    }

    pub fn require(&mut self, offset: usize) -> &mut Block {
        let id = offset_to_id(offset);
        if !self.contains(id) {
            self.put(id);
        }
        self.get_without_update(id)
    }

    fn contains(&self, id: BlockId) -> bool {
        self.blocks.contains_key(&id)
    }

    fn get_without_update(&mut self, id: BlockId) -> &mut Block {
        debug_assert!(self.contains(id));
        self.blocks.get_mut(&id).unwrap()
    }

    fn get(&mut self, id: BlockId) -> &mut Block {
        debug_assert!(self.contains(id));

        let block = self.blocks.get_mut(&id).unwrap();
        let head = self.head.unwrap();

        if head != id {
            let (next, prev) = (block.next, block.prev);

            block.next = Some(head);
            block.prev = None;

            self.get_without_update(prev.unwrap()).next = next;

            match next {
                Some(id) => self.get_without_update(id).prev = prev,
                None => self.tail = prev
            };

            self.get_without_update(head).prev = Some(id);
            self.head = Some(id);
        }

        self.get_without_update(id)
    }

    fn put(&mut self, id: BlockId) {
        if let Some(capacity) = self.capacity 
            && self.blocks.len() == capacity {
            self.remove(self.tail.unwrap());
        }
        self.push_front(id)
    }

    fn push_front(&mut self, id: BlockId) {
        let block = self.blocks
            .entry(id)
            .insert_entry(Block::new(id))
            .into_mut();

        block.next = self.head;
        if let Some(head) = self.head {
            self.get_without_update(head).prev = Some(id);
        }

        self.head = Some(id);
        if self.tail.is_none() {
            self.tail = Some(id);
        }
    }

    fn remove(&mut self, id: BlockId) {
        let Some(block) = self.blocks.get(&id) else {
            return;
        };

        let (next, prev) = (block.next, block.prev);

        match prev {
            Some(id) => self.get_without_update(id).next = next,
            None => self.head = next
        };
        
        match next {
            Some(id) => self.get_without_update(id).prev = prev,
            None => self.tail = prev
        };

        self.blocks.remove(&id);
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn block_append() {
        let first = b"first line\nsecond line\nthird line";
        let second = b"fourth line";

        let mut block = Block::new(0);

        block.append(first);
        assert_eq!(block.nlines, 3);

        block.append(second);
        assert_eq!(block.nlines, 4);

        assert_eq!(block.size, first.len() + second.len());

        let mut i = 0;
        first.iter().for_each(|c| {
            assert_eq!(block.data[i], *c);
            i += 1;
        });
        second.iter().for_each(|c| {
            assert_eq!(block.data[i], *c);
            i += 1;
        });
    }

    #[test]
    fn block_append_returns_bytes_cannot_appended() {
        let bytes = b"first line\nsecond line\nthird line";

        let mut block = Block::new(0);
        block.size = BLOCK_SIZE - 5;
        let remaining_bytes = block.append(bytes);

        assert_eq!(block.nlines, 1);
        assert_matches!(remaining_bytes, Some(_));
        assert_eq!(remaining_bytes.unwrap(), &bytes[5..])
    }

    fn cache() -> BlockCache {
        let mut cache = BlockCache::with_capacity(Some(4));

        cache.push_front(0);
        cache.push_front(1);
        cache.push_front(2);

        // 2 -> 1 -> 0
        cache
    }

    #[test]
    fn cache_add() {
        let mut cache = cache();
        assert_matches!(cache.head, Some(2));
        assert_matches!(cache.tail, Some(0));

        let mut block = cache.get_without_update(cache.head.unwrap());
        let mut next = block.next;
        assert_eq!(block.id, 2);
        assert_matches!(block.prev, None);
        assert_matches!(block.next, Some(1));

        block = cache.get_without_update(next.unwrap());
        next = block.next;
        assert_eq!(block.id, 1);
        assert_matches!(block.prev, Some(2));
        assert_matches!(block.next, Some(0));

        block = cache.get_without_update(next.unwrap());
        assert_eq!(block.id, 0);
        assert_matches!(block.prev, Some(1));
        assert_matches!(block.next, None);
    }

    #[test]
    fn cache_put() {
        let mut cache = cache();

        // 3 -> 2 -> 1 -> 0
        cache.put(3);
        assert_matches!(cache.head, Some(3));
        assert_matches!(cache.tail, Some(0));

        let block = cache.get_without_update(cache.head.unwrap());
        assert_matches!(block.prev, None);
        assert_matches!(block.next, Some(2));

        let block = cache.get_without_update(2);
        assert_matches!(block.prev, Some(3));
        assert_matches!(block.next, Some(1));

        // remove LRU
        // 4 -> 3 -> 2 -> 1
        cache.put(4);
        assert_matches!(cache.head, Some(4));
        assert_matches!(cache.tail, Some(1));

        let block = cache.get_without_update(cache.head.unwrap());
        assert_matches!(block.prev, None);
        assert_matches!(block.next, Some(3));

        let block = cache.get_without_update(1);
        assert_matches!(block.prev, Some(2));
        assert_matches!(block.next, None);
    }

    #[test]
    fn cache_get() {
        let mut cache = cache();

        // 1 -> 2 -> 0
        let block = cache.get(1);
        assert_matches!(block.prev, None);
        assert_matches!(block.next, Some(2));
        assert_matches!(cache.head, Some(1));
        assert_matches!(cache.tail, Some(0));

        let block = cache.get_without_update(2);
        assert_matches!(block.prev, Some(1));
        assert_matches!(block.next, Some(0));

        let block = cache.get_without_update(0);
        assert_matches!(block.prev, Some(2));
        assert_matches!(block.next, None);
    }

    #[test]
    fn cache_require() {
        let mut cache = cache();

        let block = cache.require(BLOCK_SIZE);
        assert_matches!(block.prev, Some(2));
        assert_matches!(block.next, Some(0));

        // 3 -> 2 -> 1 -> 0
        let block = cache.require(BLOCK_SIZE * 3);
        assert_matches!(block.prev, None);
        assert_matches!(block.next, Some(2));
    }
}
