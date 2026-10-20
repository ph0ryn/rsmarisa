/// Basic test to verify all inserted words can be found

#[test]
fn test_all_words_can_be_found() {
    use rsmarisa::{Agent, Keyset, Trie};

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

    // Build trie
    let mut keyset = Keyset::new();
    for word in &words {
        keyset.push_back_str(word).unwrap();
    }

    let mut trie = Trie::new();
    trie.build(&mut keyset, 0);

    println!("Trie built successfully:");
    println!("  num_keys: {}", trie.num_keys());
    println!("  num_nodes: {}", trie.num_nodes());
    println!();

    // Try to lookup each word
    let mut found = Vec::new();
    let mut not_found = Vec::new();

    for word in &words {
        let mut agent = Agent::new();
        agent.set_query_str(word);

        if trie.lookup(&mut agent) {
            found.push(*word);
            println!("✓ Found: {}", word);
        } else {
            not_found.push(*word);
            println!("✗ NOT FOUND: {}", word);
        }
    }

    println!();
    println!("Summary:");
    println!("  Found: {} / {}", found.len(), words.len());
    println!("  Not found: {}", not_found.len());

    if !not_found.is_empty() {
        println!();
        println!("Missing words:");
        for word in &not_found {
            println!("  - {}", word);
        }
        panic!("{} words are missing from the trie", not_found.len());
    }
}

mod tail_prefix_regressions {
    use rsmarisa::base::TailMode;
    use rsmarisa::{Agent, Keyset, Trie};
    use tempfile::NamedTempFile;

    fn build_and_load(keys: &[&[u8]], flags: i32) -> (Trie, Trie) {
        let mut keyset = Keyset::new();
        for key in keys {
            keyset.push_back_bytes(key, 1.0).unwrap();
        }
        let mut built = Trie::new();
        built.build(&mut keyset, flags);

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        built.save(path).unwrap();
        let mut loaded = Trie::new();
        loaded.load(path).unwrap();
        assert_eq!(built.num_keys(), keys.len());
        assert_eq!(loaded.num_keys(), keys.len());
        assert_eq!(loaded.tail_mode(), built.tail_mode());
        (built, loaded)
    }

    fn predictive_keys(trie: &Trie, query: &[u8]) -> Vec<Vec<u8>> {
        let mut agent = Agent::new();
        agent.set_query_bytes(query);
        let mut keys = Vec::new();
        while trie.predictive_search(&mut agent) {
            keys.push(agent.key().as_bytes().to_vec());
        }
        assert!(!trie.predictive_search(&mut agent));
        keys
    }

    #[test]
    fn text_tail_ngram_minimal_after_save_load() {
        // Rust-specific: Valid NUL-free n-gram keys must not underflow in debug builds.
        let keys: &[&[u8]] = &[
            b"\x01\x0b\x81\x01\x15\x80\x01\x01\x01\x01\x05",
            b"\x01\x0c\x81\x01\x16\x80\x01\x01\x01\x01\x03",
        ];
        let (built, loaded) = build_and_load(keys, 3 | TailMode::TextTail as i32);
        assert_eq!(loaded.tail_mode(), TailMode::TextTail);
        assert_eq!(
            predictive_keys(&loaded, b"\x01\x0b\x81"),
            vec![keys[0].to_vec()]
        );
        assert_eq!(
            predictive_keys(&built, b"\x01\x0b\x81"),
            vec![keys[0].to_vec()]
        );
    }

    #[test]
    fn text_tail_ngram_query_matrix() {
        // Rust-specific: Cover 90 registered/missing history queries before and after save/load.
        for history_len in 1..=5 {
            for num_tries in 1..=3 {
                for delimiter in [128, 129] {
                    let mut first_query = [0x01, 0x0b].repeat(history_len);
                    first_query.push(delimiter);
                    let mut second_query = [0x01, 0x0c].repeat(history_len);
                    second_query.push(delimiter);
                    let mut missing_query = [0x01, 0x0d].repeat(history_len);
                    missing_query.push(delimiter);

                    let mut first_key = first_query.clone();
                    let mut second_key = second_query.clone();
                    if delimiter == 129 {
                        first_key.extend_from_slice(b"\x01\x15\x80");
                        second_key.extend_from_slice(b"\x01\x16\x80");
                    }
                    first_key.extend_from_slice(b"\x01\x01\x01\x01\x05");
                    second_key.extend_from_slice(b"\x01\x01\x01\x01\x03");
                    let keys = [first_key.as_slice(), second_key.as_slice()];
                    let (built, loaded) =
                        build_and_load(&keys, num_tries | TailMode::TextTail as i32);
                    for trie in [&built, &loaded] {
                        assert_eq!(trie.tail_mode(), TailMode::TextTail);
                        for (query, expected) in [
                            (&first_query, vec![first_key.clone()]),
                            (&second_query, vec![second_key.clone()]),
                            (&missing_query, Vec::new()),
                        ] {
                            assert_eq!(
                                predictive_keys(trie, query),
                                expected,
                                "history_len={history_len}, num_tries={num_tries}, delimiter={delimiter}, query={query:02x?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn text_and_binary_tail_predictive_boundaries() {
        // Rust-specific: Preserve text and NUL-containing binary tail results and termination.
        for mode in [TailMode::TextTail, TailMode::BinaryTail] {
            let keys: &[&[u8]] = if mode == TailMode::TextTail {
                &[b"apple", b"banana"]
            } else {
                &[b"a\0pple", b"b\0anana"]
            };
            let (built, loaded) = build_and_load(keys, 1 | mode as i32);
            for trie in [&built, &loaded] {
                assert_eq!(trie.tail_mode(), mode);
                assert_eq!(
                    predictive_keys(trie, b""),
                    vec![keys[0].to_vec(), keys[1].to_vec()]
                );
                assert_eq!(predictive_keys(trie, b"a"), vec![keys[0].to_vec()]);
                assert_eq!(predictive_keys(trie, &keys[0][..3]), vec![keys[0].to_vec()]);
                assert_eq!(predictive_keys(trie, keys[0]), vec![keys[0].to_vec()]);
                assert!(predictive_keys(trie, b"ac").is_empty());
                assert!(predictive_keys(trie, b"c").is_empty());
                let mut overlong = keys[0].to_vec();
                overlong.push(b'!');
                assert!(predictive_keys(trie, &overlong).is_empty());
            }
        }
    }
}
