//! Rust-specific regressions for initialized public loading destinations.

use rsmarisa::grimoire::io::{Mapper, Reader, Writer};
use rsmarisa::grimoire::trie::cache::Cache;
use rsmarisa::grimoire::vector::{rank_index::RankIndex, vector::Vector};
use rsmarisa::{Agent, Keyset, Trie};
use std::cell::Cell;
use std::io::{self, Cursor, Read};
use std::rc::Rc;

fn build_trie(words: &[&str]) -> Trie {
    let mut keys = Keyset::new();
    for word in words {
        keys.push_back_str(word).unwrap();
    }
    let mut trie = Trie::new();
    trie.build(&mut keys, 1);
    trie
}

fn dictionary_bytes() -> Vec<u8> {
    let trie = build_trie(&["app", "apple", "banana", "band"]);
    let mut writer = Writer::from_vec(Vec::new());
    trie.write(&mut writer).unwrap();
    writer.into_inner().unwrap()
}

/// A legal reader may inspect all destination bytes, including on failure.
/// Supplying short reads also exercises read_exact's partially filled buffers.
struct InspectingRead<'a> {
    source: Cursor<&'a [u8]>,
    observed_bytes: Rc<Cell<usize>>,
    checksum: u8,
    fail_at: Option<usize>,
}

impl Read for InspectingRead<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        for &byte in buffer.iter() {
            self.checksum = self.checksum.wrapping_add(byte);
        }
        self.observed_bytes
            .set(self.observed_bytes.get() + buffer.len());

        if buffer.is_empty() {
            return Ok(0);
        }
        let position = self.source.position() as usize;
        let mut count = buffer.len().min(3);
        if let Some(fail_at) = self.fail_at {
            if position >= fail_at {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "injected partial read failure",
                ));
            }
            count = count.min(fail_at - position);
        }
        self.source.read(&mut buffer[..count])
    }
}

#[test]
fn trie_read_allows_legal_reader_to_inspect_destination() {
    // Rust-specific: safe Read implementations can observe old buffer contents.
    // Miri detects the original Vector::set_len-before-initialization bug here.
    let bytes = dictionary_bytes();
    let observed_bytes = Rc::new(Cell::new(0));
    let source = InspectingRead {
        source: Cursor::new(&bytes),
        observed_bytes: observed_bytes.clone(),
        checksum: 0,
        fail_at: None,
    };
    let mut reader = Reader::from_reader(source);
    let mut trie = Trie::new();
    trie.read(&mut reader).unwrap();

    assert!(observed_bytes.get() >= bytes.len());
    assert_eq!(trie.num_keys(), 4);
    let mut agent = Agent::new();
    agent.set_query_str("apple");
    assert!(trie.lookup(&mut agent));
}

#[test]
fn trie_read_truncated_prefix_initializes_destination_before_read() {
    // Rust-specific: isolate the load path from Keyset/build/Writer lifetimes.
    // This is a valid MARISA header and one u64 vector, truncated immediately
    // afterward. The reader must see initialized bytes even for invalid input.
    let mut bytes = b"We love Marisa.\0".to_vec();
    bytes.extend_from_slice(&8u64.to_ne_bytes());
    bytes.extend_from_slice(&0u64.to_ne_bytes());
    let observed_bytes = Rc::new(Cell::new(0));
    let source = InspectingRead {
        source: Cursor::new(&bytes),
        observed_bytes: observed_bytes.clone(),
        checksum: 0,
        fail_at: None,
    };
    let mut reader = Reader::from_reader(source);
    let mut trie = Trie::new();

    let error = trie.read(&mut reader).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert!(observed_bytes.get() >= bytes.len());
}

#[test]
fn trie_read_partial_failure_preserves_existing_trie() {
    // Rust-specific: after some payload bytes have been written, a legal reader
    // can inspect the rest, fail, and leave the previously loaded trie intact.
    let bytes = dictionary_bytes();
    // MARISA starts with a 16-byte header and a u64 vector byte length.
    let payload_start = 16 + std::mem::size_of::<u64>();
    let payload_len = u64::from_ne_bytes(bytes[16..payload_start].try_into().unwrap());
    assert!(payload_len > 3);
    let fail_at = payload_start + 3;
    let observed_bytes = Rc::new(Cell::new(0));
    let source = InspectingRead {
        source: Cursor::new(&bytes),
        observed_bytes: observed_bytes.clone(),
        checksum: 0,
        fail_at: Some(fail_at),
    };
    let mut reader = Reader::from_reader(source);
    let mut trie = build_trie(&["retained"]);
    let error = trie.read(&mut reader).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(error.to_string(), "injected partial read failure");
    assert!(observed_bytes.get() > fail_at);
    assert_eq!(trie.num_keys(), 1);
    let mut agent = Agent::new();
    agent.set_query_str("retained");
    assert!(trie.lookup(&mut agent));
}

#[test]
fn trie_load_truncated_file_preserves_existing_trie() {
    // Rust-specific: file-backed partial reads return EOF without replacing the
    // existing public Trie or leaving typed destinations uninitialized on drop.
    let bytes = dictionary_bytes();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("truncated.marisa");
    std::fs::write(&path, &bytes[..27]).unwrap();
    let mut trie = build_trie(&["retained"]);

    let error = trie.load(path.to_str().unwrap()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(trie.num_keys(), 1);
    let mut agent = Agent::new();
    agent.set_query_str("retained");
    assert!(trie.lookup(&mut agent));
}

#[test]
fn writer_preserves_bool_and_char_support() {
    // Rust-specific: output needs NoUninit, not arbitrary-bit-valid Pod.
    let mut writer = Writer::from_vec(Vec::new());
    writer.write(&true).unwrap();
    writer.write_slice(&['a', 'λ']).unwrap();
    let mut expected = vec![1];
    expected.extend_from_slice(&('a' as u32).to_ne_bytes());
    expected.extend_from_slice(&('λ' as u32).to_ne_bytes());
    assert_eq!(writer.into_inner().unwrap(), expected);

    let mut flags = Vector::new();
    flags.push_back(true);
    flags.push_back(false);
    let mut writer = Writer::from_vec(Vec::new());
    flags.write(&mut writer).unwrap();
    let mut expected = 2u64.to_ne_bytes().to_vec();
    expected.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(writer.into_inner().unwrap(), expected);
}

#[test]
fn vector_partial_read_leaves_initialized_elements() {
    // Rust-specific: reading every element remains valid after an incomplete
    // payload; the unwritten element must already have an initialized value.
    let mut bytes = 8u64.to_ne_bytes().to_vec();
    bytes.extend_from_slice(&7u32.to_ne_bytes());
    // Cursor::read_exact may reject short input before copying anything. Use
    // short reads to guarantee the first element is written before EOF.
    let source = InspectingRead {
        source: Cursor::new(&bytes),
        observed_bytes: Rc::new(Cell::new(0)),
        checksum: 0,
        fail_at: None,
    };
    let mut vector = Vector::<u32>::new();
    let error = vector.read(&mut Reader::from_reader(source)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(vector.as_slice(), &[7, 0]);
}

#[test]
fn vector_rejects_fractional_elements_for_read_and_map() {
    // Rust-specific: do not silently floor a serialized byte count.
    static BYTE_LENGTH: [u8; 8] = 3u64.to_ne_bytes();
    let mut read_vector = Vector::<u32>::new();
    let error = read_vector
        .read(&mut Reader::from_bytes(&BYTE_LENGTH))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    let mut mapped_vector = Vector::<u32>::new();
    let error = mapped_vector
        .map(&mut Mapper::open_memory(&BYTE_LENGTH))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn vector_partial_map_leaves_initialized_elements() {
    // Rust-specific: insufficient mapped data must not expose uninitialized T.
    static TRUNCATED: [u8; 8] = 8u64.to_ne_bytes();
    let mut vector = Vector::<u32>::new();
    let error = vector
        .map(&mut Mapper::open_memory(&TRUNCATED))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(vector.as_slice(), &[0, 0]);
}

#[test]
fn cache_and_rank_index_preserve_native_field_order() {
    // Rust-specific: repr(C)/Pod must retain the C++ three-word file layout.
    let mut cache = Cache::new();
    cache.set_parent(0x11223344);
    cache.set_child(0x55667788);
    cache.set_extra(0xABCDEF);
    cache.set_base(0x12);
    let mut writer = Writer::from_vec(Vec::new());
    writer.write(&cache).unwrap();
    let mut expected = Vec::new();
    for word in [0x11223344u32, 0x55667788, 0xABCDEF12] {
        expected.extend_from_slice(&word.to_ne_bytes());
    }
    assert_eq!(writer.into_inner().unwrap(), expected);
    let loaded: Cache = Reader::from_bytes(&expected).read().unwrap();
    assert_eq!(loaded.parent(), cache.parent());
    assert_eq!(loaded.child(), cache.child());
    assert_eq!(loaded.link(), cache.link());

    let mut ranks = RankIndex::new();
    ranks.set_abs(0x11223344);
    ranks.set_rel1(1);
    ranks.set_rel2(2);
    ranks.set_rel3(3);
    ranks.set_rel4(4);
    ranks.set_rel5(5);
    ranks.set_rel6(6);
    ranks.set_rel7(7);
    let mut writer = Writer::from_vec(Vec::new());
    writer.write(&ranks).unwrap();
    let mut expected = Vec::new();
    for word in [
        0x11223344u32,
        1 | (2 << 7) | (3 << 15) | (4 << 23),
        5 | (6 << 9) | (7 << 18),
    ] {
        expected.extend_from_slice(&word.to_ne_bytes());
    }
    assert_eq!(writer.into_inner().unwrap(), expected);
    let loaded: RankIndex = Reader::from_bytes(&expected).read().unwrap();
    assert_eq!(loaded.abs(), ranks.abs());
    assert_eq!(loaded.rel1(), ranks.rel1());
    assert_eq!(loaded.rel7(), ranks.rel7());
}
