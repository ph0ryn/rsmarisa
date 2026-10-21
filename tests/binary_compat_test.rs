//! Binary compatibility test with C++ marisa-trie
//!
//! This test verifies that Rust-generated binary files are identical to
//! C++-generated files when built from the same keyset.

use rsmarisa::grimoire::io::Reader;
use rsmarisa::{Agent, Keyset, Trie};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

#[test]
#[ignore] // Run manually with: cargo test binary_compat -- --ignored
fn test_binary_compatibility_with_cpp() {
    // Create temporary directory for test files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let rust_file = temp_dir.path().join("rust_output.marisa");
    let cpp_file = temp_dir.path().join("cpp_output.marisa");

    // Build Rust trie
    let mut keyset = Keyset::new();
    let words = vec![
        "a",
        "app",
        "apple",
        "application",
        "apply",
        "banana",
        "band",
        "bank",
        "can",
        "cat",
        "dog",
        "door",
        "test",
        "testing",
        "trie",
    ];

    for word in &words {
        keyset.push_back_str(word).unwrap();
    }

    let mut trie = Trie::new();
    trie.build(&mut keyset, 0);

    println!("Rust trie stats:");
    println!("  Keys: {}", trie.num_keys());
    println!("  Nodes: {}", trie.num_nodes());
    println!("  I/O size: {} bytes", trie.io_size());

    // Save Rust trie
    trie.save(rust_file.to_str().unwrap())
        .expect("Failed to save Rust trie");

    // Build C++ trie if tools are available
    let cpp_test_exe = find_cpp_test_binary();
    if let Some(cpp_exe) = cpp_test_exe {
        println!("Running C++ test program: {}", cpp_exe.display());
        let output = Command::new(&cpp_exe)
            .arg(cpp_file.to_str().unwrap())
            .output()
            .expect("Failed to run C++ test program");

        if !output.status.success() {
            panic!(
                "C++ test program failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        println!("{}", String::from_utf8_lossy(&output.stdout));

        // Compare files
        let rust_data = fs::read(&rust_file).expect("Failed to read Rust file");
        let cpp_data = fs::read(&cpp_file).expect("Failed to read C++ file");

        println!("\nFile sizes:");
        println!("  Rust: {} bytes", rust_data.len());
        println!("  C++:  {} bytes", cpp_data.len());

        if rust_data == cpp_data {
            println!("\n✓ Binary files are identical!");
        } else {
            println!("\n✗ Binary files differ!");

            // Show first difference
            for (i, (r, c)) in rust_data.iter().zip(cpp_data.iter()).enumerate() {
                if r != c {
                    println!(
                        "First difference at byte {}: Rust=0x{:02x}, C++=0x{:02x}",
                        i, r, c
                    );
                    break;
                }
            }

            if rust_data.len() != cpp_data.len() {
                println!(
                    "Length difference: {} bytes",
                    (rust_data.len() as i64 - cpp_data.len() as i64).abs()
                );
            }

            panic!("Binary compatibility test failed");
        }
    } else {
        println!("C++ test binary not found - skipping comparison");
        println!("To enable full test:");
        println!("1. Build C++ marisa-trie");
        println!("2. Compile tests/cpp_binary_test.cc");
        println!("3. Place it in target/debug/ or target/release/");
    }
}

fn find_cpp_test_binary() -> Option<PathBuf> {
    // Look for cpp_test in build directories
    let candidates = vec![
        PathBuf::from("target/debug/cpp_test"),
        PathBuf::from("target/release/cpp_test"),
        PathBuf::from("/tmp/cpp_test"),
    ];

    candidates.into_iter().find(|path| path.exists())
}

// Rust-specific: malformed serialized flags must be I/O errors, not panics,
// and a failed replacement must leave the existing trie available.
fn assert_invalid_configuration_rejected(
    mut load: impl FnMut(&mut Trie, &[u8]) -> std::io::Result<()>,
) {
    let directory = TempDir::new().unwrap();
    let valid_path = directory.path().join("valid.marisa");
    let mut keys = Keyset::new();
    keys.push_back_str("apple").unwrap();
    keys.push_back_str("banana").unwrap();
    let mut source = Trie::new();
    source.build(&mut keys, 1);
    source.save(valid_path.to_str().unwrap()).unwrap();
    let valid = fs::read(valid_path).unwrap();
    // The outer trie's configuration is the final serialized u32.
    let offset = valid.len() - 4;
    let flags = u32::from_le_bytes(valid[offset..].try_into().unwrap());
    let invalid_flags = [
        ("unknown flags", flags | 0x100000),
        ("undefined cache level", (flags & !0x00F80) | 0x00900),
        ("undefined tail mode", (flags & !0x0F000) | 0x0C000),
        ("undefined node order", (flags & !0xF0000) | 0xC0000),
        ("negative flags", u32::MAX),
    ];

    for (case, flags) in invalid_flags {
        let mut bytes = valid.clone();
        bytes[offset..].copy_from_slice(&flags.to_le_bytes());
        let mut existing_keys = Keyset::new();
        existing_keys.push_back_str("sentinel").unwrap();
        let mut trie = Trie::new();
        trie.build(&mut existing_keys, 1);

        let error = load(&mut trie, &bytes).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData, "{case}");
        let mut agent = Agent::new();
        agent.set_query_str("sentinel");
        assert!(trie.lookup(&mut agent), "failed load replaced trie: {case}");
    }
}

#[test]
fn rejects_invalid_configuration_on_load() {
    // Rust-specific: exercise the file-loading public boundary.
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("invalid.marisa");
    assert_invalid_configuration_rejected(|trie, bytes| {
        fs::write(&path, bytes)?;
        trie.load(path.to_str().unwrap())
    });
}

#[test]
fn rejects_invalid_configuration_on_read() {
    // Rust-specific: the same validation applies to caller-provided readers.
    assert_invalid_configuration_rejected(|trie, bytes| trie.read(&mut Reader::from_bytes(bytes)));
}

#[test]
fn rejects_invalid_configuration_on_map() {
    // Rust-specific: Trie::map requires static backing storage, so each small
    // malformed test buffer is retained until this test process exits.
    assert_invalid_configuration_rejected(|trie, bytes| {
        trie.map(Box::leak(bytes.to_vec().into_boxed_slice()))
    });
}

#[test]
#[cfg(feature = "mmap")]
fn rejects_invalid_configuration_on_mmap() {
    // Rust-specific: mapped-file loading must reject the flags as well.
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("invalid.marisa");
    assert_invalid_configuration_rejected(|trie, bytes| {
        fs::write(&path, bytes)?;
        // SAFETY: This test owns the private temporary file and starts no writers
        // from before this call until its mapping is dropped. Invalid flags make
        // the call fail and drop its temporary mapping before returning, so the
        // next iteration's write cannot modify a live mapping. The existing trie
        // is built, not mapped.
        unsafe { trie.mmap(path.to_str().unwrap()) }
    });
}
