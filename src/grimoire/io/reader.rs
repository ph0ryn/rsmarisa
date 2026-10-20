//! Reader for deserializing trie data.
//!
//! Ported from:
//! - lib/marisa/grimoire/io/reader.h
//! - lib/marisa/grimoire/io/reader.cc
//!
//! Reader provides methods to read binary data from various sources
//! including files, byte slices, and any type implementing std::io::Read.

use bytemuck::Pod;
use std::fs::File;
use std::io::{self, Read as IoRead};
use std::path::Path;

/// Reader for reading binary data from various sources.
///
/// Reader wraps a std::io::Read implementation and provides convenient
/// methods for reading typed data and seeking forward.
pub struct Reader<'a> {
    /// The underlying reader, boxed for trait object support.
    reader: Option<Box<dyn IoRead + 'a>>,
}

impl<'a> Reader<'a> {
    /// Creates a new empty reader.
    pub fn new() -> Self {
        Reader { reader: None }
    }

    /// Opens a file for reading.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to open
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Reader<'static>> {
        let file = File::open(path)?;
        Ok(Reader {
            reader: Some(Box::new(file)),
        })
    }

    /// Creates a reader from any type implementing std::io::Read.
    ///
    /// # Arguments
    ///
    /// * `reader` - Any type implementing Read
    pub fn from_reader<R: IoRead + 'a>(reader: R) -> Self {
        Reader {
            reader: Some(Box::new(reader)),
        }
    }

    /// Creates a reader from a byte slice.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Byte slice to read from
    pub fn from_bytes(bytes: &[u8]) -> Reader<'static> {
        Reader {
            reader: Some(Box::new(io::Cursor::new(bytes.to_vec()))),
        }
    }

    /// Reads and returns a single value of type T.
    ///
    /// # Errors
    ///
    /// Returns an error if the reader is not open or if reading fails.
    ///
    /// Reads the native memory representation. `Pod` excludes invalid bit
    /// patterns and padding, unlike `Copy` alone.
    ///
    /// ```compile_fail
    /// use rsmarisa::grimoire::io::Reader;
    /// let mut reader = Reader::from_bytes(&[2]);
    /// let _: bool = reader.read().unwrap();
    /// ```
    ///
    /// ```compile_fail
    /// use rsmarisa::grimoire::io::Reader;
    /// #[repr(u8)]
    /// #[derive(Clone, Copy)]
    /// enum Flag { Off = 0, On = 1 }
    /// let mut reader = Reader::from_bytes(&[2]);
    /// let _: Flag = reader.read().unwrap();
    /// ```
    pub fn read<T: Pod>(&mut self) -> io::Result<T> {
        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Reader not open"))?;

        // Safe Read implementations may inspect the entire destination buffer.
        let mut value = T::zeroed();
        reader.read_exact(bytemuck::bytes_of_mut(&mut value))?;
        Ok(value)
    }

    /// Reads a single value of type T into a mutable reference.
    ///
    /// # Arguments
    ///
    /// * `value` - Mutable reference to store the read value
    ///
    /// # Errors
    ///
    /// Returns an error if the reader is not open or if reading fails.
    ///
    /// `Pod` keeps the destination valid even if a partial read fails.
    ///
    /// ```compile_fail
    /// use rsmarisa::grimoire::io::Reader;
    /// let mut reader = Reader::from_bytes(&[2]);
    /// reader.read_into(&mut false).unwrap();
    /// ```
    pub fn read_into<T: Pod>(&mut self, value: &mut T) -> io::Result<()> {
        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Reader not open"))?;

        reader.read_exact(bytemuck::bytes_of_mut(value))?;
        Ok(())
    }

    /// Reads multiple values into a slice.
    ///
    /// # Arguments
    ///
    /// * `values` - Mutable slice to store the read values
    ///
    /// # Errors
    ///
    /// Returns an error if the reader is not open or if reading fails.
    ///
    /// Elements must be initialized before this call; `Pod` ensures arbitrary
    /// input bytes cannot invalidate them, including on partial failure.
    ///
    /// ```compile_fail
    /// use rsmarisa::grimoire::io::Reader;
    /// let mut reader = Reader::from_bytes(&[2]);
    /// reader.read_slice(&mut [false]).unwrap();
    /// ```
    pub fn read_slice<T: Pod>(&mut self, values: &mut [T]) -> io::Result<()> {
        if values.is_empty() {
            return Ok(());
        }

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Reader not open"))?;

        reader.read_exact(bytemuck::cast_slice_mut(values))?;
        Ok(())
    }

    /// Seeks forward by the specified number of bytes.
    ///
    /// This method skips the specified number of bytes by reading and
    /// discarding them. For small sizes (<= 16 bytes), it uses a stack
    /// buffer. For larger sizes, it uses a 1024-byte buffer.
    ///
    /// # Arguments
    ///
    /// * `size` - Number of bytes to skip
    ///
    /// # Errors
    ///
    /// Returns an error if the reader is not open or if reading fails.
    pub fn seek(&mut self, size: usize) -> io::Result<()> {
        if size == 0 {
            return Ok(());
        }

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "Reader not open"))?;

        if size <= 16 {
            let mut buf = [0u8; 16];
            reader.read_exact(&mut buf[..size])?;
        } else {
            let mut buf = [0u8; 1024];
            let mut remaining = size;
            while remaining > 0 {
                let count = remaining.min(buf.len());
                reader.read_exact(&mut buf[..count])?;
                remaining -= count;
            }
        }
        Ok(())
    }

    /// Checks if the reader is open.
    pub fn is_open(&self) -> bool {
        self.reader.is_some()
    }

    /// Closes the reader and releases resources.
    pub fn clear(&mut self) {
        self.reader = None;
    }
}

impl<'a> Default for Reader<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reader_new() {
        let reader = Reader::new();
        assert!(!reader.is_open());
    }

    #[test]
    fn test_reader_from_bytes() {
        let data = vec![1u8, 2, 3, 4, 5];
        let reader = Reader::from_bytes(&data);
        assert!(reader.is_open());
    }

    #[test]
    fn test_reader_read_u32() {
        let data = vec![0x01, 0x02, 0x03, 0x04];
        let mut reader = Reader::from_bytes(&data);

        let value: u32 = reader.read().unwrap();

        // Little-endian: 0x04030201
        assert_eq!(value, 0x04030201);
    }

    #[test]
    fn test_reader_read_slice() {
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut reader = Reader::from_bytes(&data);

        let mut values = [0u8; 4];
        reader.read_slice(&mut values).unwrap();

        assert_eq!(values, [1, 2, 3, 4]);

        // Read next 4 bytes
        reader.read_slice(&mut values).unwrap();
        assert_eq!(values, [5, 6, 7, 8]);
    }

    #[test]
    fn test_reader_read_empty_slice() {
        let data = vec![1u8, 2, 3, 4];
        let mut reader = Reader::from_bytes(&data);

        let mut values: [u8; 0] = [];
        reader.read_slice(&mut values).unwrap();
    }

    #[test]
    fn test_reader_seek() {
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut reader = Reader::from_bytes(&data);

        // Skip 4 bytes
        reader.seek(4).unwrap();

        let value: u8 = reader.read().unwrap();
        assert_eq!(value, 5);
    }

    #[test]
    fn test_reader_seek_zero() {
        let data = vec![1u8, 2, 3, 4];
        let mut reader = Reader::from_bytes(&data);

        reader.seek(0).unwrap();

        let value: u8 = reader.read().unwrap();
        assert_eq!(value, 1);
    }

    #[test]
    fn test_reader_seek_large() {
        let data = vec![0u8; 2048];
        let mut reader = Reader::from_bytes(&data);

        // Seek past 1024 bytes (tests large buffer path)
        reader.seek(1500).unwrap();

        let value: u8 = reader.read().unwrap();
        assert_eq!(value, 0);
    }

    #[test]
    fn test_reader_clear() {
        let data = vec![1u8, 2, 3, 4];
        let mut reader = Reader::from_bytes(&data);

        assert!(reader.is_open());
        reader.clear();
        assert!(!reader.is_open());
    }

    #[test]
    fn test_reader_not_open() {
        let mut reader = Reader::new();

        let result = reader.read::<u32>();
        assert!(result.is_err());
    }

    #[test]
    fn test_reader_from_reader() {
        let data = vec![1u8, 2, 3, 4];
        let cursor = io::Cursor::new(data);
        let mut reader = Reader::from_reader(cursor);

        assert!(reader.is_open());

        let value: u32 = reader.read().unwrap();
        assert_eq!(value, 0x04030201);
    }

    #[test]
    fn test_reader_read_multiple_types() {
        let mut data = Vec::new();
        data.extend_from_slice(&42u32.to_le_bytes());
        data.extend_from_slice(&100u64.to_le_bytes());

        let mut reader = Reader::from_bytes(&data);

        let val_u32: u32 = reader.read().unwrap();
        assert_eq!(val_u32, 42);

        let val_u64: u64 = reader.read().unwrap();
        assert_eq!(val_u64, 100);
    }

    #[test]
    fn test_reader_insufficient_data() {
        let data = vec![1u8, 2];
        let mut reader = Reader::from_bytes(&data);

        let result = reader.read::<u32>();
        assert!(result.is_err());
    }

    #[test]
    fn test_reader_default() {
        let reader = Reader::default();
        assert!(!reader.is_open());
    }
}
