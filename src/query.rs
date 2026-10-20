//! Query type for trie searches.
//!
//! Ported from: include/marisa/query.h

use std::fmt;

/// Query represents a search query with a string and optional ID.
///
/// A query can contain either:
/// - An owned string to search for
/// - An ID to reverse lookup
///
/// Unlike the original C++ query, setter inputs are copied so safe access and
/// cloning do not depend on the caller keeping its input alive or unchanged.
#[derive(Clone)]
pub struct Query {
    /// Owned query bytes, if a string is set.
    bytes: Option<Vec<u8>>,
    /// Query ID for reverse lookup.
    id: usize,
}

// Keep query contents out of the debug representation.
impl fmt::Debug for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Query")
            .field("ptr", &self.ptr().map(|_| "..."))
            .field("length", &self.length())
            .field("id", &self.id)
            .finish()
    }
}

impl Default for Query {
    fn default() -> Self {
        Self::new()
    }
}

impl Query {
    /// Creates a new empty query.
    pub fn new() -> Self {
        Query { bytes: None, id: 0 }
    }

    /// Returns the character at the specified index.
    ///
    /// # Panics
    ///
    /// Panics if index is out of bounds.
    pub fn get(&self, i: usize) -> u8 {
        assert!(i < self.length(), "Index out of bounds");
        self.as_bytes()[i]
    }

    /// Copies a string slice into the query's owned storage.
    pub fn set_str(&mut self, s: &str) {
        let bytes = self.bytes.get_or_insert_with(Vec::new);
        bytes.clear();
        bytes.extend_from_slice(s.as_bytes());
    }

    /// Copies a byte slice into the query's owned storage.
    pub fn set_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            self.bytes = None;
        } else {
            let storage = self.bytes.get_or_insert_with(Vec::new);
            storage.clear();
            storage.extend_from_slice(bytes);
        }
    }

    /// Sets the query ID.
    pub fn set_id(&mut self, id: usize) {
        self.id = id;
    }

    /// Returns the query as a byte slice.
    ///
    /// Returns an empty slice if no string is set.
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_deref().unwrap_or(&[])
    }

    /// Returns the query string as a str reference.
    ///
    /// # Panics
    ///
    /// Panics if the query contains invalid UTF-8.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.as_bytes()).expect("Invalid UTF-8 in query")
    }

    /// Returns a pointer into the query's owned data.
    ///
    /// The pointer may be invalidated by mutating or dropping the query.
    pub fn ptr(&self) -> Option<*const u8> {
        self.bytes.as_ref().map(|bytes| bytes.as_ptr())
    }

    /// Returns the length of the query string.
    pub fn length(&self) -> usize {
        self.as_bytes().len()
    }

    /// Returns the query ID.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Clears the query to empty state.
    pub fn clear(&mut self) {
        *self = Query::new();
    }

    /// Swaps with another query.
    pub fn swap(&mut self, other: &mut Query) {
        std::mem::swap(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_new() {
        let query = Query::new();
        assert_eq!(query.length(), 0);
        assert_eq!(query.id(), 0);
        assert_eq!(query.as_bytes(), &[]);
    }

    #[test]
    fn test_query_default() {
        let query = Query::default();
        assert_eq!(query.length(), 0);
    }

    #[test]
    fn test_query_set_str() {
        let s = "hello";
        let mut query = Query::new();
        query.set_str(s);

        assert_eq!(query.length(), 5);
        assert_eq!(query.as_str(), "hello");
        assert_eq!(query.as_bytes(), b"hello");
    }

    #[test]
    fn test_query_set_bytes() {
        let bytes = b"world";
        let mut query = Query::new();
        query.set_bytes(bytes);

        assert_eq!(query.length(), 5);
        assert_eq!(query.as_bytes(), b"world");
    }

    #[test]
    fn test_query_set_empty_bytes() {
        let mut query = Query::new();
        query.set_str("test");
        query.set_bytes(&[]);

        assert_eq!(query.length(), 0);
        assert_eq!(query.as_bytes(), &[]);
    }

    #[test]
    fn test_query_get() {
        let s = "test";
        let mut query = Query::new();
        query.set_str(s);

        assert_eq!(query.get(0), b't');
        assert_eq!(query.get(1), b'e');
        assert_eq!(query.get(2), b's');
        assert_eq!(query.get(3), b't');
    }

    #[test]
    #[should_panic(expected = "Index out of bounds")]
    fn test_query_get_out_of_bounds() {
        let s = "test";
        let mut query = Query::new();
        query.set_str(s);
        query.get(4);
    }

    #[test]
    fn test_query_set_id() {
        let mut query = Query::new();
        query.set_id(42);
        assert_eq!(query.id(), 42);
    }

    #[test]
    fn test_query_clear() {
        let mut query = Query::new();
        query.set_str("test");
        query.set_id(10);

        query.clear();

        assert_eq!(query.length(), 0);
        assert_eq!(query.id(), 0);
        assert_eq!(query.as_bytes(), &[]);
    }

    #[test]
    fn test_query_swap() {
        let s1 = "hello";
        let s2 = "world";

        let mut q1 = Query::new();
        q1.set_str(s1);
        q1.set_id(1);

        let mut q2 = Query::new();
        q2.set_str(s2);
        q2.set_id(2);

        q1.swap(&mut q2);

        assert_eq!(q1.as_str(), "world");
        assert_eq!(q1.id(), 2);
        assert_eq!(q2.as_str(), "hello");
        assert_eq!(q2.id(), 1);
    }

    #[test]
    fn test_query_with_unicode() {
        let s = "こんにちは";
        let mut query = Query::new();
        query.set_str(s);

        assert_eq!(query.length(), s.len());
        assert_eq!(query.as_str(), "こんにちは");
    }
}
