//! Memory-mapped file access.
//!
//! Ported from:
//! - lib/marisa/grimoire/io/mapper.h
//! - lib/marisa/grimoire/io/mapper.cc
//!
//! Mapper provides read-only access to data through memory mapping.
//! This implementation supports both file-backed memory mapping and borrowed memory.
//!
//! When the `mmap` feature is disabled (e.g., for WASM), only borrowed memory
//! mode is available. File-backed memory mapping requires the `mmap` feature.
//!
//! ## WASM Usage
//!
//! On WASM targets, memory mapping is unavailable. Instead, load trie data
//! from the host environment and pass it as a byte slice:
//!
//! - **Browser**: Fetch the `.marisa` file as `Uint8Array` via `fetch()`,
//!   then pass the buffer into WASM → `Mapper::open_memory()`.
//! - **WASI**: Read the file via WASI filesystem APIs into a `Vec<u8>`,
//!   then use `Mapper::open_memory()`.
//! - **Embedded (WAMR)**: The host copies trie data into WASM linear memory,
//!   and the module accesses it via `Mapper::open_memory()`.
//!
//! See also: [WAMR host data sharing](https://bytecodealliance.github.io/wamr.dev/blog/the-wasm-host-sharing-data-basics/)

use bytemuck::Pod;
#[cfg(feature = "mmap")]
use memmap2::Mmap;
use std::io;

/// Mapper for memory-mapped data access.
///
/// Mapper provides read-only access to data, primarily used for
/// deserializing trie structures from memory or files.
///
/// The mapper can work in two modes:
/// - File-backed memory mapping using `memmap2::Mmap` (requires `mmap` feature)
/// - Borrowed memory slices (for testing or in-memory data)
pub struct Mapper {
    /// File-backed memory map (only available with `mmap` feature).
    #[cfg(feature = "mmap")]
    mmap: Option<Mmap>,
    /// Borrowed memory reference (static lifetime for safety).
    borrowed: Option<&'static [u8]>,
    /// Current read position.
    position: usize,
}

impl Default for Mapper {
    fn default() -> Self {
        Self::new()
    }
}

impl Mapper {
    /// Creates a new empty mapper.
    pub fn new() -> Self {
        Mapper {
            #[cfg(feature = "mmap")]
            mmap: None,
            borrowed: None,
            position: 0,
        }
    }

    /// Opens a mapper from a file using memory mapping.
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the file to map
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or mapped.
    /// On WASM (without the `mmap` feature), always returns an error.
    ///
    /// # Safety
    ///
    /// You must ensure that the backing file is not modified or truncated by
    /// any handle or process, from before this call until the mapping is dropped.
    /// This obligation follows the mapping if the returned mapper is moved or
    /// swapped into another owner. Opening the file read-only does not prevent
    /// changes through other handles or processes.
    ///
    /// Use [`Reader`](super::Reader) instead if you cannot guarantee file stability.
    ///
    /// File-backed mapping requires an explicit unsafe call:
    ///
    /// ```compile_fail,E0133
    /// use rsmarisa::grimoire::io::Mapper;
    /// let _ = Mapper::open_file("dictionary.marisa");
    /// ```
    #[cfg(feature = "mmap")]
    pub unsafe fn open_file(filename: &str) -> io::Result<Self> {
        use std::fs::File;
        let file = File::open(filename)?;
        // SAFETY: The caller guarantees file stability from before this call
        // until the mapping is dropped, including after ownership transfers.
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Mapper {
            mmap: Some(mmap),
            borrowed: None,
            position: 0,
        })
    }

    /// Opens a mapper from a file.
    ///
    /// Without the `mmap` feature, file-backed mapping is not available.
    /// Returns an error indicating mmap is not supported.
    ///
    /// # Safety
    ///
    /// There are no additional safety requirements when `mmap` is disabled:
    /// this stub creates no mapping. Its signature stays unsafe so that enabling
    /// the feature does not silently add obligations to a safe call.
    ///
    /// The unsafe call requirement is consistent across feature configurations:
    ///
    /// ```compile_fail,E0133
    /// use rsmarisa::grimoire::io::Mapper;
    /// let _ = Mapper::open_file("dictionary.marisa");
    /// ```
    #[cfg(not(feature = "mmap"))]
    pub unsafe fn open_file(filename: &str) -> io::Result<Self> {
        let _ = filename;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "File-backed memory mapping requires the 'mmap' feature (not available on WASM)",
        ))
    }

    /// Opens a mapper from a byte slice.
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice to map (must have static lifetime)
    ///
    /// # Safety
    ///
    /// The data must have a valid static lifetime to ensure it outlives
    /// the mapper and any structures that reference it.
    pub fn open_memory(data: &'static [u8]) -> Self {
        Mapper {
            #[cfg(feature = "mmap")]
            mmap: None,
            borrowed: Some(data),
            position: 0,
        }
    }

    /// Opens a mapper from a byte slice (legacy method).
    ///
    /// # Arguments
    ///
    /// * `data` - Byte slice to map (must have static lifetime)
    ///
    /// # Note
    ///
    /// This method is provided for compatibility. New code should use
    /// `open_memory()` instead.
    pub fn open(data: &'static [u8]) -> Self {
        Self::open_memory(data)
    }

    /// Returns a reference to the underlying data.
    ///
    /// Returns the data from either the memory map or borrowed slice.
    fn data(&self) -> &[u8] {
        #[cfg(feature = "mmap")]
        if let Some(ref m) = self.mmap {
            return &m[..];
        }
        self.borrowed.unwrap_or(&[])
    }

    /// Maps a single value of type T from the current position.
    ///
    /// # Arguments
    ///
    /// * `value` - Mutable reference to store the mapped value
    ///
    /// # Errors
    ///
    /// Returns an error if the mapper is not open or if there's insufficient data.
    ///
    /// `Pod` ensures that arbitrary input bytes form a valid value without
    /// padding or pointer invariants.
    ///
    /// ```compile_fail
    /// use rsmarisa::grimoire::io::Mapper;
    /// let mut mapper = Mapper::open_memory(&[2]);
    /// mapper.map(&mut false).unwrap();
    /// ```
    pub fn map<T: Pod>(&mut self, value: &mut T) -> io::Result<()> {
        let data = self.data();
        if data.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Mapper not open",
            ));
        }

        let size = std::mem::size_of::<T>();
        if size > data.len() - self.position {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Insufficient data to map",
            ));
        }

        let slice = &data[self.position..self.position + size];
        bytemuck::bytes_of_mut(value).copy_from_slice(slice);

        self.position += size;
        Ok(())
    }

    /// Maps and returns a single value of type T from the current position.
    ///
    /// Convenience method that returns the value instead of taking a mutable reference.
    ///
    /// # Errors
    ///
    /// Returns an error if the mapper is not open or if there's insufficient data.
    ///
    /// Only types with an arbitrary-bit-valid, padding-free representation
    /// implement `Pod`.
    ///
    /// ```compile_fail
    /// use rsmarisa::grimoire::io::Mapper;
    /// let mut mapper = Mapper::open_memory(&[2]);
    /// let _: bool = mapper.map_value().unwrap();
    /// ```
    pub fn map_value<T: Pod + Default>(&mut self) -> io::Result<T> {
        let mut value = T::default();
        self.map(&mut value)?;
        Ok(value)
    }

    /// Maps a slice of values from the current position.
    ///
    /// # Arguments
    ///
    /// * `values` - Mutable slice to fill with mapped values
    ///
    /// # Errors
    ///
    /// Returns an error if the mapper is not open or if there's insufficient data.
    ///
    /// `Pod` ensures every copied byte pattern is valid for each element.
    ///
    /// ```compile_fail
    /// use rsmarisa::grimoire::io::Mapper;
    /// let mut mapper = Mapper::open_memory(&[2]);
    /// mapper.map_slice(&mut [false]).unwrap();
    /// ```
    pub fn map_slice<T: Pod>(&mut self, values: &mut [T]) -> io::Result<()> {
        if values.is_empty() {
            return Ok(());
        }

        let data = self.data();
        if data.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Mapper not open",
            ));
        }

        let size = std::mem::size_of_val(values);
        if size > data.len() - self.position {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Insufficient data to map",
            ));
        }

        let slice = &data[self.position..self.position + size];
        bytemuck::cast_slice_mut(values).copy_from_slice(slice);

        self.position += size;
        Ok(())
    }

    /// Seeks forward by the specified number of bytes.
    ///
    /// # Arguments
    ///
    /// * `size` - Number of bytes to skip
    ///
    /// # Errors
    ///
    /// Returns an error if the mapper is not open or if seeking past the end.
    pub fn seek(&mut self, size: usize) -> io::Result<()> {
        let data = self.data();
        if data.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Mapper not open",
            ));
        }

        if size > data.len() - self.position {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Seek past end of data",
            ));
        }

        self.position += size;
        Ok(())
    }

    /// Checks if the mapper is open.
    pub fn is_open(&self) -> bool {
        #[cfg(feature = "mmap")]
        if self.mmap.is_some() {
            return true;
        }
        self.borrowed.is_some()
    }

    /// Returns the current position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns the total size of mapped data.
    pub fn size(&self) -> usize {
        self.data().len()
    }

    /// Closes the mapper.
    pub fn clear(&mut self) {
        #[cfg(feature = "mmap")]
        {
            self.mmap = None;
        }
        self.borrowed = None;
        self.position = 0;
    }

    /// Swaps with another mapper.
    pub fn swap(&mut self, other: &mut Mapper) {
        #[cfg(feature = "mmap")]
        std::mem::swap(&mut self.mmap, &mut other.mmap);
        std::mem::swap(&mut self.borrowed, &mut other.borrowed);
        std::mem::swap(&mut self.position, &mut other.position);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "mmap")]
    use std::io::Write;
    #[cfg(feature = "mmap")]
    use tempfile::NamedTempFile;

    #[test]
    fn test_mapper_new() {
        let mapper = Mapper::new();
        assert!(!mapper.is_open());
        assert_eq!(mapper.position(), 0);
    }

    #[test]
    fn test_mapper_open_memory() {
        static DATA: [u8; 5] = [1, 2, 3, 4, 5];
        let mapper = Mapper::open_memory(&DATA);
        assert!(mapper.is_open());
        assert_eq!(mapper.size(), 5);
        assert_eq!(mapper.position(), 0);
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapper_open_file() {
        // Create a temporary file
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = vec![1u8, 2, 3, 4, 5];
        temp_file.write_all(&test_data).unwrap();
        temp_file.flush().unwrap();

        let path = temp_file.path().to_str().unwrap();
        // SAFETY: The test owns this temporary file and does not modify or
        // truncate it from this call until the mapper is dropped.
        let mapper = unsafe { Mapper::open_file(path) }.unwrap();

        assert!(mapper.is_open());
        assert_eq!(mapper.size(), 5);
        assert_eq!(mapper.position(), 0);
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapper_open_file_not_found() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing.dat");
        // SAFETY: This test exclusively owns the directory and never creates
        // the file, so the call cannot create a mapping.
        let result = unsafe { Mapper::open_file(path.to_str().unwrap()) };
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::NotFound);
    }

    #[cfg(not(feature = "mmap"))]
    #[test]
    fn test_mapper_file_mapping_unsupported() {
        // Rust-specific: Disabling mmap keeps the API unsafe and returns Unsupported.
        // SAFETY: This feature configuration never creates a file mapping.
        let result = unsafe { Mapper::open_file("unused.marisa") };
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn test_mapper_map_u32() {
        static DATA: [u8; 4] = [0x01, 0x02, 0x03, 0x04];
        let mut mapper = Mapper::open_memory(&DATA);

        let mut value: u32 = 0;
        mapper.map(&mut value).unwrap();

        // Little-endian: 0x04030201
        assert_eq!(value, 0x04030201);
        assert_eq!(mapper.position(), 4);
    }

    #[test]
    fn test_mapper_map_value() {
        static DATA: [u8; 4] = [0x01, 0x02, 0x03, 0x04];
        let mut mapper = Mapper::open_memory(&DATA);

        let value: u32 = mapper.map_value().unwrap();

        // Little-endian: 0x04030201
        assert_eq!(value, 0x04030201);
        assert_eq!(mapper.position(), 4);
    }

    #[test]
    fn test_mapper_map_slice() {
        static DATA: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut mapper = Mapper::open_memory(&DATA);

        let mut values = [0u8; 4];
        mapper.map_slice(&mut values).unwrap();

        assert_eq!(values, [1, 2, 3, 4]);
        assert_eq!(mapper.position(), 4);

        // Map next 4 bytes
        mapper.map_slice(&mut values).unwrap();
        assert_eq!(values, [5, 6, 7, 8]);
        assert_eq!(mapper.position(), 8);
    }

    #[test]
    fn test_mapper_map_empty_slice() {
        static DATA: [u8; 4] = [1, 2, 3, 4];
        let mut mapper = Mapper::open_memory(&DATA);

        let mut values: [u8; 0] = [];
        mapper.map_slice(&mut values).unwrap();

        assert_eq!(mapper.position(), 0);
    }

    #[test]
    fn test_mapper_seek() {
        static DATA: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let mut mapper = Mapper::open_memory(&DATA);

        mapper.seek(4).unwrap();
        assert_eq!(mapper.position(), 4);

        let mut value: u8 = 0;
        mapper.map(&mut value).unwrap();
        assert_eq!(value, 5);
    }

    #[test]
    fn test_mapper_seek_zero() {
        static DATA: [u8; 4] = [1, 2, 3, 4];
        let mut mapper = Mapper::open_memory(&DATA);

        mapper.seek(0).unwrap();
        assert_eq!(mapper.position(), 0);
    }

    #[test]
    fn test_mapper_clear() {
        static DATA: [u8; 4] = [1, 2, 3, 4];
        let mut mapper = Mapper::open_memory(&DATA);

        assert!(mapper.is_open());
        mapper.clear();
        assert!(!mapper.is_open());
        assert_eq!(mapper.position(), 0);
    }

    #[test]
    fn test_mapper_swap() {
        static DATA1: [u8; 3] = [1, 2, 3];
        static DATA2: [u8; 5] = [4, 5, 6, 7, 8];

        let mut mapper1 = Mapper::open_memory(&DATA1);
        let mut mapper2 = Mapper::open_memory(&DATA2);

        mapper1.seek(1).unwrap();
        mapper2.seek(2).unwrap();

        assert_eq!(mapper1.size(), 3);
        assert_eq!(mapper2.size(), 5);
        assert_eq!(mapper1.position(), 1);
        assert_eq!(mapper2.position(), 2);

        mapper1.swap(&mut mapper2);

        assert_eq!(mapper1.size(), 5);
        assert_eq!(mapper2.size(), 3);
        assert_eq!(mapper1.position(), 2);
        assert_eq!(mapper2.position(), 1);
    }

    #[test]
    fn test_mapper_not_open() {
        let mut mapper = Mapper::new();
        let mut value: u32 = 0;

        let result = mapper.map(&mut value);
        assert!(result.is_err());
    }

    #[test]
    fn test_mapper_insufficient_data() {
        static DATA: [u8; 2] = [1, 2];
        let mut mapper = Mapper::open_memory(&DATA);

        let mut value: u32 = 0;
        let result = mapper.map(&mut value);
        assert!(result.is_err());
    }

    #[test]
    fn test_mapper_seek_past_end() {
        static DATA: [u8; 4] = [1, 2, 3, 4];
        let mut mapper = Mapper::open_memory(&DATA);

        let result = mapper.seek(10);
        assert!(result.is_err());
    }

    #[test]
    fn test_mapper_multiple_maps() {
        // Create static data with u32 and u64 values
        static DATA: [u8; 12] = [
            42, 0, 0, 0, // 42 as u32 (little-endian)
            100, 0, 0, 0, 0, 0, 0, 0, // 100 as u64 (little-endian)
        ];

        let mut mapper = Mapper::open_memory(&DATA);

        let mut val_u32: u32 = 0;
        mapper.map(&mut val_u32).unwrap();
        assert_eq!(val_u32, 42);

        let mut val_u64: u64 = 0;
        mapper.map(&mut val_u64).unwrap();
        assert_eq!(val_u64, 100);
    }

    #[test]
    fn test_mapper_default() {
        let mapper = Mapper::default();
        assert!(!mapper.is_open());
    }

    #[cfg(feature = "mmap")]
    #[test]
    fn test_mapper_file_mapping() {
        // Create a temporary file with some data
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = vec![0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        temp_file.write_all(&test_data).unwrap();
        temp_file.flush().unwrap();

        let path = temp_file.path().to_str().unwrap();
        // SAFETY: The test owns this temporary file and does not modify or
        // truncate it from this call until the mapper is dropped.
        let mut mapper = unsafe { Mapper::open_file(path) }.unwrap();

        // Test reading from file-backed mmap
        let mut values = [0u8; 4];
        mapper.map_slice(&mut values).unwrap();
        assert_eq!(values, [1, 2, 3, 4]);

        mapper.map_slice(&mut values).unwrap();
        assert_eq!(values, [5, 6, 7, 8]);
    }
}
