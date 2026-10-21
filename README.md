# rsmarisa

[![CI](https://github.com/tokuhirom/rsmarisa/actions/workflows/ci.yml/badge.svg)](https://github.com/tokuhirom/rsmarisa/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/rsmarisa.svg)](https://crates.io/crates/rsmarisa)
[![Documentation](https://docs.rs/rsmarisa/badge.svg)](https://docs.rs/rsmarisa)
[![License](https://img.shields.io/badge/license-BSD--2--Clause-blue.svg)](LICENSE)

Pure Rust port of [marisa-trie](https://github.com/s-yata/marisa-trie), a static and space-efficient trie data structure.

## About

MARISA (Matching Algorithm with Recursively Implemented StorAge) is a static and space-efficient trie data structure. This is a **pure Rust** implementation (no C++ dependencies) that maintains full binary compatibility with the original C++ marisa-trie while leveraging Rust's safety features.

**Why rsmarisa?**

- Pure Rust implementation - no C++ dependencies or FFI overhead
- Binary compatible with C++ marisa-trie (can read/write the same files)
- Memory safe with comprehensive test coverage (321 tests)
- Identical behavior to the original C++ library
- File-backed memory mapping and static-memory loading support

## Features

- **Lookup**: Check whether a given string exists in the dictionary
- **Reverse lookup**: Restore a key from its ID
- **Common prefix search**: Find keys from prefixes of a given string
- **Predictive search**: Find keys starting with a given string
- **Space-efficient**: Compressed trie structure with LOUDS encoding
- **Binary I/O**: Save and load tries to/from files
- **Memory-mapped I/O**: File-backed mapping with an explicit unsafe caller contract
- **Full binary compatibility**: Rust-built tries are byte-for-byte identical to C++-built tries

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
rsmarisa = "0.2"
```

### Basic Usage

```rust
use rsmarisa::{Trie, Keyset, Agent};

// Build a trie
let mut keyset = Keyset::new();
keyset.push_back_str("app").unwrap();
keyset.push_back_str("apple").unwrap();
keyset.push_back_str("application").unwrap();

let mut trie = Trie::new();
trie.build(&mut keyset, 0);

// Lookup
let mut agent = Agent::new();
agent.init_state().unwrap();
agent.set_query_str("apple");
assert!(trie.lookup(&mut agent));

// Common prefix search
agent.set_query_str("application");
while trie.common_prefix_search(&mut agent) {
    println!("Found prefix: {}", agent.key().as_str());
}
// Prints: "app", "application"
```

### Save and Load

```rust
use rsmarisa::{Trie, Keyset};

// Build and save
let mut keyset = Keyset::new();
keyset.push_back_str("hello").unwrap();
keyset.push_back_str("world").unwrap();

let mut trie = Trie::new();
trie.build(&mut keyset, 0);
trie.save("dictionary.marisa").unwrap();

// Load
let mut loaded_trie = Trie::new();
loaded_trie.load("dictionary.marisa").unwrap();
```

### Memory-Mapped I/O

File-backed mapping requires the default-on `mmap` feature and an unsafe call.
The caller must prevent every process from modifying or truncating the backing
file from before the call until the mapping is actually dropped. This includes
opening, mapping, and parsing the file. Moving or swapping the owning trie
transfers this obligation; read-only file access alone does not enforce it.

Only use the file-backed example below when your application's storage policy
provides that guarantee. Otherwise, use safe `load()` instead.

```rust
use rsmarisa::Trie;

let mut mapped_trie = Trie::new();
// SAFETY: The application's storage policy prevents any process from modifying
// or truncating this file from before this call until the mapping is dropped,
// including after any move or swap of mapped_trie.
unsafe { mapped_trie.mmap("dictionary.marisa") }.unwrap();

// Embedding static data does not require unsafe.
static DICT_DATA: &[u8] = include_bytes!("dictionary.marisa");
let mut embedded_trie = Trie::new();
embedded_trie.map(DICT_DATA).unwrap();
```

**When to use `mmap()`, `load()`, or `map()`:**

- **Ordinary file input**: Use safe `load()` without a mapping-lifetime guarantee.
- **Controlled immutable files**: Consider unsafe `mmap()` only when you can
  maintain the backing-file guarantee until the mapping is actually dropped.
- **Embedded data**: Use safe `map()` with `include_bytes!()`.

These methods use the same dictionary format and support the same search
operations. The current `map()`/`mmap()` implementation copies payloads into owned
vectors: trie loading is not zero-copy or O(1), and the file mapping remains
retained by its owner even after the copies finish.

## Examples

Run the included examples:

```bash
# Basic usage demonstration
cargo run --example basic_usage

# Save/load file I/O
cargo run --example save_load
```

## Status

✅ **Production ready!** Core functionality is complete with full binary compatibility and all CLI tools working.

**Latest Updates (v0.2.0 - 2026-01-27):**

- ✅ **Memory-mapped I/O**: Added file-backed `Trie::mmap()` loading; calls now require an explicit unsafe contract
- ✅ **Owned payloads**: The mapping path copies into owned vectors rather than providing zero-copy or O(1) loading
- ✅ **Static memory support**: `Trie::map()` for embedding dictionary data in binaries
- ✅ **Binary compatibility**: Both `mmap()` and `load()` work with C++-created files

**Upcoming in v0.3.0 (Breaking Change):**

- 🔄 **Library name alignment**: The library name has been changed from `marisa` to `rsmarisa` to match the package name
  - **Migration required**: Update all imports from `use marisa::` to `use rsmarisa::`
  - **Why**: Eliminates confusion and aligns with Rust naming conventions
  - **Impact**: All users must update their code when upgrading to v0.3.0

See [CHANGELOG.md](CHANGELOG.md) for complete version history.

### What's Implemented

- ✅ **LOUDS trie construction** - Space-efficient trie building
- ✅ **Lookup operations** - Exact string matching
- ✅ **Common prefix search** - Find all prefixes of a string
- ✅ **Binary I/O** - Save/load with C++ marisa-trie compatibility
- ✅ **Memory-mapped I/O** - File-backed loading with owned payload copies
- ✅ **File format validation** - "We love Marisa." header check
- ✅ **321 comprehensive tests** - All passing ✅

### Compatibility

| Feature | Status | Notes |
|---------|--------|-------|
| Behavioral compatibility | ✅ | Search operations match C++ exactly |
| Binary file format | ✅ | Byte-for-byte identical to C++ output |
| C++ file reading | ✅ | Can read files from C++ marisa-trie |
| C++ file writing | ✅ | C++ can read Rust-built files |
| Cross-verification | ✅ | Verified with `cmp` and `marisa-lookup` |
| Memory-mapped I/O | ✅ | Implemented using memmap2 |

### Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_trie_lookup
```

## CLI Tools

Command-line tools with `rsmarisa-` prefix (to avoid conflicts with C++ marisa-trie):

**Available:**

- ✅ `rsmarisa-build` - Build a dictionary from text input (binary compatible with C++)
- ✅ `rsmarisa-lookup` - Look up keys in a dictionary (results match C++ exactly)
- ✅ `rsmarisa-common-prefix-search` - Find keys that are prefixes of queries (results match C++ exactly)
- ✅ `rsmarisa-dump` - Dump dictionary contents (results match C++ exactly)
- ✅ `rsmarisa-predictive-search` - Find keys with a given prefix (results match C++ exactly)
- ✅ `rsmarisa-reverse-lookup` - Restore keys from IDs (results match C++ exactly)

**Coming Soon:**

- `rsmarisa-benchmark` - Performance benchmarking

All CLI tools are now fully functional and produce output identical to their C++ counterparts.

### Usage Examples

```bash
# Build a dictionary
echo -e "apple\nbanana\ncherry" | cargo run --release --bin rsmarisa-build -- -o dict.marisa

# Look up keys
echo -e "apple\ngrape" | cargo run --release --bin rsmarisa-lookup -- dict.marisa
# Output: 0\tapple
#         -1\tgrape

# Find common prefixes
echo "application" | cargo run --release --bin rsmarisa-common-prefix-search -- dict.marisa
# Output: 3 found
#         0\ta\tapplication
#         1\tapp\tapplication
#         5\tapplication\tapplication
```

## Performance

MARISA tries are designed to be:

- **Space-efficient**: Uses LOUDS encoding and suffix compression
- **Fast lookups**: O(m) where m is the query length
- **Cache-friendly**: Sequential memory access patterns

See [PORTING_STATUS.md](PORTING_STATUS.md) for detailed implementation progress.

## Original Project

- **Repository**: <https://github.com/s-yata/marisa-trie>
- **Author**: Susumu Yata
- **Version**: 0.3.1
- **Baseline commit**: `4ef33cc5a2b6b4f5e147e4564a5236e163d67982`

## Contributing

Contributions are welcome! Please see [CLAUDE.md](CLAUDE.md) for porting guidelines and project structure.

### Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run clippy
cargo clippy

# Format code
cargo fmt
```

## License

BSD-2-Clause (same as the original project)

See [LICENSE](LICENSE) for details.

## Acknowledgments

This is a Rust port of the excellent [marisa-trie](https://github.com/s-yata/marisa-trie) library by Susumu Yata.
