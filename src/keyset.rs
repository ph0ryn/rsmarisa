//! Keyset for collecting keys to build a trie.
//!
//! Ported from:
//! - include/marisa/keyset.h
//! - lib/marisa/keyset.cc

use crate::key::Key;
use std::io;

/// Number of Key objects per allocation block.
const KEY_BLOCK_SIZE: usize = 256;

/// Keyset collects keys for trie construction.
///
/// Key objects are stored in reusable blocks. Unlike the original C++ keyset,
/// each key owns its string so mutable access can safely move it out of the set.
pub struct Keyset {
    /// Blocks of Key objects, each owning its bytes.
    key_blocks: Vec<Box<[Key; KEY_BLOCK_SIZE]>>,
    /// Total number of keys.
    size: usize,
    /// Total length of all key strings.
    total_length: usize,
}

impl Default for Keyset {
    fn default() -> Self {
        Self::new()
    }
}

impl Keyset {
    /// Creates a new empty keyset.
    pub fn new() -> Self {
        Keyset {
            key_blocks: Vec::new(),
            size: 0,
            total_length: 0,
        }
    }

    /// Adds a key to the keyset.
    pub fn push_back_key(&mut self, key: &Key) {
        let new_key = self.next_key_mut();
        new_key.set_bytes(key.as_bytes());
        new_key.set_id(key.id());

        self.size += 1;
        self.total_length += key.length();
    }

    /// Adds a key with an end marker character outside its logical length.
    pub fn push_back_key_with_marker(&mut self, key: &Key, end_marker: u8) {
        let new_key = self.next_key_mut();
        new_key.set_bytes_with_marker(key.as_bytes(), end_marker);
        new_key.set_id(key.id());

        self.size += 1;
        self.total_length += key.length();
    }

    /// Adds a string to the keyset with default weight of 1.0.
    pub fn push_back_str(&mut self, s: &str) -> io::Result<()> {
        self.push_back_bytes(s.as_bytes(), 1.0)
    }

    /// Adds bytes to the keyset with specified weight.
    pub fn push_back_bytes(&mut self, bytes: &[u8], weight: f32) -> io::Result<()> {
        if bytes.len() > u32::MAX as usize {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Key too long"));
        }

        let key = self.next_key_mut();
        key.set_bytes(bytes);
        key.set_weight(weight);

        self.size += 1;
        self.total_length += bytes.len();

        Ok(())
    }

    /// Returns a reference to the key at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if index is out of bounds.
    pub fn get(&self, i: usize) -> &Key {
        assert!(i < self.size, "Index out of bounds");
        &self.key_blocks[i / KEY_BLOCK_SIZE][i % KEY_BLOCK_SIZE]
    }

    /// Returns a mutable reference to the key at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if index is out of bounds.
    pub fn get_mut(&mut self, i: usize) -> &mut Key {
        assert!(i < self.size, "Index out of bounds");
        &mut self.key_blocks[i / KEY_BLOCK_SIZE][i % KEY_BLOCK_SIZE]
    }

    /// Returns the number of keys in the keyset.
    pub fn num_keys(&self) -> usize {
        self.size
    }

    /// Returns true if the keyset is empty.
    pub fn empty(&self) -> bool {
        self.size == 0
    }

    /// Returns the number of keys (alias for num_keys).
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the total length of all key strings.
    pub fn total_length(&self) -> usize {
        self.total_length
    }

    /// Resets the keyset to reuse allocated memory.
    pub fn reset(&mut self) {
        self.size = 0;
        self.total_length = 0;
        // Keep allocated blocks for reuse
    }

    /// Clears all data and frees memory.
    pub fn clear(&mut self) {
        *self = Keyset::new();
    }

    /// Swaps with another keyset.
    pub fn swap(&mut self, other: &mut Keyset) {
        std::mem::swap(self, other);
    }

    /// Returns the next reusable owned Key slot, allocating a block if needed.
    fn next_key_mut(&mut self) -> &mut Key {
        if self.size / KEY_BLOCK_SIZE == self.key_blocks.len() {
            self.append_key_block();
        }
        &mut self.key_blocks[self.size / KEY_BLOCK_SIZE][self.size % KEY_BLOCK_SIZE]
    }

    /// Appends a new key block.
    fn append_key_block(&mut self) {
        // Create a new block with default Keys
        let block = Box::new([(); KEY_BLOCK_SIZE].map(|_| Key::new()));
        self.key_blocks.push(block);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyset_new() {
        let keyset = Keyset::new();
        assert_eq!(keyset.size(), 0);
        assert_eq!(keyset.total_length(), 0);
        assert!(keyset.empty());
    }

    #[test]
    fn test_keyset_default() {
        let keyset = Keyset::default();
        assert_eq!(keyset.size(), 0);
    }

    #[test]
    fn test_keyset_push_back_str() {
        let mut keyset = Keyset::new();

        keyset.push_back_str("hello").unwrap();
        keyset.push_back_str("world").unwrap();

        assert_eq!(keyset.size(), 2);
        assert_eq!(keyset.total_length(), 10);
        assert_eq!(keyset.get(0).as_str(), "hello");
        assert_eq!(keyset.get(1).as_str(), "world");
    }

    #[test]
    fn test_keyset_push_back_bytes() {
        let mut keyset = Keyset::new();

        keyset.push_back_bytes(b"test", 2.5).unwrap();

        assert_eq!(keyset.size(), 1);
        assert_eq!(keyset.get(0).as_bytes(), b"test");
        assert!((keyset.get(0).weight() - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_keyset_push_back_key() {
        let mut keyset = Keyset::new();
        let s = "example";

        let mut key = Key::new();
        key.set_str(s);
        key.set_id(42);

        keyset.push_back_key(&key);

        assert_eq!(keyset.size(), 1);
        assert_eq!(keyset.get(0).as_str(), "example");
        assert_eq!(keyset.get(0).id(), 42);
    }

    #[test]
    fn test_keyset_push_back_key_with_marker() {
        let mut keyset = Keyset::new();
        let s = "test";

        let mut key = Key::new();
        key.set_str(s);
        key.set_id(10);

        keyset.push_back_key_with_marker(&key, b'\0');

        assert_eq!(keyset.size(), 1);
        assert_eq!(keyset.get(0).as_str(), "test");
        // End marker is not included in the key length
    }

    #[test]
    fn test_keyset_get_mut() {
        let mut keyset = Keyset::new();
        keyset.push_back_str("hello").unwrap();

        {
            let key = keyset.get_mut(0);
            key.set_id(99);
        }

        assert_eq!(keyset.get(0).id(), 99);
    }

    #[test]
    fn test_keyset_reset() {
        let mut keyset = Keyset::new();
        keyset.push_back_str("hello").unwrap();
        keyset.push_back_str("world").unwrap();

        assert_eq!(keyset.size(), 2);

        keyset.reset();

        assert_eq!(keyset.size(), 0);
        assert_eq!(keyset.total_length(), 0);
    }

    #[test]
    fn test_keyset_clear() {
        let mut keyset = Keyset::new();
        keyset.push_back_str("hello").unwrap();

        keyset.clear();

        assert_eq!(keyset.size(), 0);
        assert_eq!(keyset.total_length(), 0);
    }

    #[test]
    fn test_keyset_swap() {
        let mut ks1 = Keyset::new();
        ks1.push_back_str("one").unwrap();

        let mut ks2 = Keyset::new();
        ks2.push_back_str("two").unwrap();
        ks2.push_back_str("three").unwrap();

        ks1.swap(&mut ks2);

        assert_eq!(ks1.size(), 2);
        assert_eq!(ks1.get(0).as_str(), "two");
        assert_eq!(ks2.size(), 1);
        assert_eq!(ks2.get(0).as_str(), "one");
    }

    #[test]
    fn test_keyset_many_keys() {
        let mut keyset = Keyset::new();

        // Push enough keys to trigger multiple blocks
        for i in 0..1000 {
            keyset.push_back_str(&format!("key{}", i)).unwrap();
        }

        assert_eq!(keyset.size(), 1000);

        // Verify some keys
        assert_eq!(keyset.get(0).as_str(), "key0");
        assert_eq!(keyset.get(500).as_str(), "key500");
        assert_eq!(keyset.get(999).as_str(), "key999");
    }

    #[test]
    fn test_keyset_large_string() {
        let mut keyset = Keyset::new();

        // String larger than EXTRA_BLOCK_SIZE
        let large_str = "x".repeat(2000);
        keyset.push_back_str(&large_str).unwrap();

        assert_eq!(keyset.size(), 1);
        assert_eq!(keyset.get(0).as_str(), large_str);
    }

    #[test]
    #[should_panic(expected = "Index out of bounds")]
    fn test_keyset_get_out_of_bounds() {
        let keyset = Keyset::new();
        keyset.get(0);
    }

    #[test]
    fn test_keyset_empty() {
        let mut keyset = Keyset::new();
        assert!(keyset.empty());

        keyset.push_back_str("test").unwrap();
        assert!(!keyset.empty());
    }
}
