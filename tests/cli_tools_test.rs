//! Integration tests for CLI tools
//!
//! Verifies that rsmarisa-build and rsmarisa-lookup produce
//! results identical to C++ marisa-trie tools.

use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use tempfile::NamedTempFile;

fn run_with_input(command: &mut Command, input: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("Failed to spawn {command:?}: {err}"));

    let mut stdin = child.stdin.take().unwrap();
    let write_result = stdin.write_all(input.as_bytes());
    // Send EOF before waiting: builders finish reading keys before writing files.
    drop(stdin);

    let output = child
        .wait_with_output()
        .expect("Failed to wait for CLI tool");
    assert!(
        output.status.success(),
        "Command {command:?} failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    write_result.expect("Failed to write to CLI tool stdin");
    output
}

#[test]
#[should_panic(expected = "failed with")]
fn test_cli_nonzero_exit_is_rejected() {
    // Rust-specific: failed tools must not pass by producing identical empty output.
    run_with_input(
        Command::new("target/release/rsmarisa-build").args(["--num-tries", "0"]),
        "",
    );
}

#[test]
fn test_rsmarisa_build_binary_compatibility() {
    // Create test input
    let test_keys = "a\napp\napple\napplication\napply\nbanana\nband\n";

    // Build with Rust tool
    let rust_dict = NamedTempFile::new().unwrap();
    run_with_input(
        Command::new("target/release/rsmarisa-build")
            .arg("-o")
            .arg(rust_dict.path()),
        test_keys,
    );

    // Build with C++ tool
    let cpp_dict = NamedTempFile::new().unwrap();
    run_with_input(
        Command::new("marisa-build").arg("-o").arg(cpp_dict.path()),
        test_keys,
    );

    // Compare binary files after both builders have exited successfully.
    let rust_bytes = fs::read(rust_dict.path()).unwrap();
    let cpp_bytes = fs::read(cpp_dict.path()).unwrap();

    assert_eq!(
        rust_bytes.len(),
        cpp_bytes.len(),
        "Dictionary files have different sizes"
    );

    assert_eq!(
        rust_bytes, cpp_bytes,
        "Dictionary files are not byte-for-byte identical"
    );
}

#[test]
fn test_rsmarisa_lookup_compatibility() {
    // Create test dictionary
    let test_keys = "a\napp\napple\napplication\napply\nbanana\nband\n";
    let dict_file = NamedTempFile::new().unwrap();

    run_with_input(
        Command::new("marisa-build").arg("-o").arg(dict_file.path()),
        test_keys,
    );

    // Test queries
    let queries = "a\napple\nbanana\nbandit\nzebra\n";

    // Lookup with Rust tool
    let rust_result = run_with_input(
        Command::new("target/release/rsmarisa-lookup").arg(dict_file.path()),
        queries,
    );
    let rust_stdout = String::from_utf8(rust_result.stdout).unwrap();

    // Lookup with C++ tool
    let cpp_result = run_with_input(Command::new("marisa-lookup").arg(dict_file.path()), queries);
    let cpp_stdout = String::from_utf8(cpp_result.stdout).unwrap();

    assert_eq!(
        rust_stdout, cpp_stdout,
        "Lookup results differ between Rust and C++ implementations"
    );
}

#[test]
fn test_rsmarisa_common_prefix_search_compatibility() {
    // Create test dictionary
    let test_keys = "a\napp\napple\napplication\napply\nbanana\nband\n";
    let dict_file = NamedTempFile::new().unwrap();

    run_with_input(
        Command::new("marisa-build").arg("-o").arg(dict_file.path()),
        test_keys,
    );

    // Test query
    let query = "application\n";

    // Search with Rust tool
    let rust_result = run_with_input(
        Command::new("target/release/rsmarisa-common-prefix-search").arg(dict_file.path()),
        query,
    );
    let rust_stdout = String::from_utf8(rust_result.stdout).unwrap();

    // Search with C++ tool
    let cpp_result = run_with_input(
        Command::new("marisa-common-prefix-search").arg(dict_file.path()),
        query,
    );
    let cpp_stdout = String::from_utf8(cpp_result.stdout).unwrap();

    assert_eq!(
        rust_stdout, cpp_stdout,
        "Common prefix search results differ between Rust and C++ implementations"
    );
}

#[test]
fn test_rsmarisa_predictive_search_compatibility() {
    // Create test dictionary
    let test_keys = "a\napp\napple\napplication\napply\nbanana\nband\n";
    let dict_file = NamedTempFile::new().unwrap();

    run_with_input(
        Command::new("marisa-build").arg("-o").arg(dict_file.path()),
        test_keys,
    );

    // Test query
    let query = "ap\n";

    // Search with Rust tool
    let rust_result = run_with_input(
        Command::new("target/release/rsmarisa-predictive-search").arg(dict_file.path()),
        query,
    );
    let rust_stdout = String::from_utf8(rust_result.stdout).unwrap();

    // Search with C++ tool
    let cpp_result = run_with_input(
        Command::new("marisa-predictive-search").arg(dict_file.path()),
        query,
    );
    let cpp_stdout = String::from_utf8(cpp_result.stdout).unwrap();

    assert_eq!(
        rust_stdout, cpp_stdout,
        "Predictive search results differ between Rust and C++ implementations"
    );
}

#[test]
fn test_rsmarisa_reverse_lookup_compatibility() {
    // Create test dictionary
    let test_keys = "a\napp\napple\napplication\napply\nbanana\nband\n";
    let dict_file = NamedTempFile::new().unwrap();

    run_with_input(
        Command::new("marisa-build").arg("-o").arg(dict_file.path()),
        test_keys,
    );

    // Test queries (IDs 0-6)
    let queries = "0\n1\n2\n3\n4\n5\n6\n";

    // Search with Rust tool
    let rust_result = run_with_input(
        Command::new("target/release/rsmarisa-reverse-lookup").arg(dict_file.path()),
        queries,
    );
    let rust_stdout = String::from_utf8(rust_result.stdout).unwrap();

    // Search with C++ tool
    let cpp_result = run_with_input(
        Command::new("marisa-reverse-lookup").arg(dict_file.path()),
        queries,
    );
    let cpp_stdout = String::from_utf8(cpp_result.stdout).unwrap();

    assert_eq!(
        rust_stdout, cpp_stdout,
        "Reverse lookup results differ between Rust and C++ implementations"
    );
}
