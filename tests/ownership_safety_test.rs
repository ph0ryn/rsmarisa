//! Rust-specific regressions for ownership at the safe public API boundary.
//!
//! These tests deliberately drop or replace setter inputs and former owners.
//! Run dangling-pointer regressions under Miri: an ordinary allocator can hide
//! use-after-free even when a value assertion happens to pass.

use rsmarisa::{Agent, Key, Keyset, Query, Trie};

fn sample_trie() -> Trie {
    let mut keyset = Keyset::new();
    for word in ["a", "app", "apple", "apply", "banana"] {
        keyset.push_back_str(word).unwrap();
    }
    let mut trie = Trie::new();
    trie.build(&mut keyset, 0);
    trie
}

#[test]
fn owned_values_preserve_send_and_sync() {
    // Rust-specific: Owning storage must preserve the existing thread traits.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Query>();
    assert_send_sync::<Key>();
    assert_send_sync::<Agent>();
}

#[test]
fn empty_setter_pointer_presence_is_preserved() {
    // Rust-specific: Owned storage preserves the empty string/byte distinction.
    let mut query = Query::new();
    let mut key = Key::new();
    assert!(query.ptr().is_none());
    assert!(key.ptr().is_none());
    query.set_str("");
    key.set_str("");
    assert!(query.ptr().is_some());
    assert!(key.ptr().is_some());
    assert!(query.as_bytes().is_empty());
    assert!(key.as_bytes().is_empty());
    query.set_bytes(&[]);
    key.set_bytes(&[]);
    assert!(query.ptr().is_none());
    assert!(key.ptr().is_none());
}

#[test]
fn query_str_survives_input_drop() {
    // Rust-specific: A safe setter cannot impose an untracked input lifetime.
    let mut query = Query::new();
    let input = String::from("query string");
    query.set_str(&input);
    drop(input);
    assert_eq!(query.as_str(), "query string");
    assert_eq!(query.get(0), b'q');
}

#[test]
fn query_bytes_survive_input_drop() {
    // Rust-specific: Binary setter input may be freed immediately after the call.
    let mut query = Query::new();
    let input = vec![0, 128, 255, 42];
    query.set_bytes(&input);
    drop(input);
    assert_eq!(query.as_bytes(), &[0, 128, 255, 42]);
}

#[test]
fn query_input_mutation_does_not_change_snapshot() {
    // Rust-specific: Mutating caller storage must not mutate a stored query.
    let mut input = b"original".to_vec();
    let mut query = Query::new();
    query.set_bytes(&input);
    input.fill(b'x');
    assert_eq!(query.as_bytes(), b"original");
}

#[test]
fn query_clone_survives_original_and_input_drop() {
    // Rust-specific: Clone must retain its own valid string storage.
    let cloned = {
        let input = String::from("cloned query");
        let mut original = Query::new();
        original.set_str(&input);
        original.set_id(17);
        original.clone()
    };
    assert_eq!(cloned.as_str(), "cloned query");
    assert_eq!(cloned.id(), 17);
}

#[test]
fn query_clone_is_independent_of_original_reassignment() {
    // Rust-specific: Reassigning the original must not invalidate its clone.
    let input = String::from("first query");
    let mut original = Query::new();
    original.set_str(&input);
    let cloned = original.clone();
    original.set_bytes(&vec![b'z'; 4096]);
    drop(input);
    drop(original);
    assert_eq!(cloned.as_str(), "first query");
}

#[test]
fn key_str_survives_input_drop() {
    // Rust-specific: Safe Key setters must own the bytes they expose later.
    let mut key = Key::new();
    let input = String::from("key string");
    key.set_str(&input);
    drop(input);
    assert_eq!(key.as_str(), "key string");
    assert_eq!(key.get(0), b'k');
}

#[test]
fn key_bytes_survive_input_drop() {
    // Rust-specific: Non-UTF-8 key bytes need the same ownership guarantee.
    let mut key = Key::new();
    let input = vec![255, 0, 128, 42];
    key.set_bytes(&input);
    drop(input);
    assert_eq!(key.as_bytes(), &[255, 0, 128, 42]);
}

#[test]
fn key_input_mutation_does_not_change_snapshot() {
    // Rust-specific: The Key's contents cannot follow mutable caller storage.
    let mut input = b"original".to_vec();
    let mut key = Key::new();
    key.set_bytes(&input);
    input.fill(b'x');
    assert_eq!(key.as_bytes(), b"original");
}

#[test]
fn key_clone_survives_original_and_input_drop() {
    // Rust-specific: Key Clone must preserve both owned bytes and metadata.
    let cloned = {
        let input = String::from("cloned key");
        let mut original = Key::new();
        original.set_str(&input);
        original.set_id(23);
        original.clone()
    };
    assert_eq!(cloned.as_str(), "cloned key");
    assert_eq!(cloned.id(), 23);
}

#[test]
fn key_clone_is_independent_of_original_reassignment() {
    // Rust-specific: A clone must not depend on the original Key's allocation.
    let input = String::from("first key");
    let mut original = Key::new();
    original.set_str(&input);
    let cloned = original.clone();
    original.set_bytes(&vec![b'z'; 4096]);
    drop(input);
    drop(original);
    assert_eq!(cloned.as_str(), "first key");
}

#[test]
fn agent_setters_survive_input_drop() {
    // Rust-specific: Agent forwarding setters cannot retain borrowed inputs.
    let mut agent = Agent::new();
    let query_input = b"agent query".to_vec();
    let key_input = String::from("agent key");
    agent.set_query_bytes(&query_input);
    agent.set_key_str(&key_input);
    drop(query_input);
    drop(key_input);
    assert_eq!(agent.query().as_bytes(), b"agent query");
    assert_eq!(agent.key().as_str(), "agent key");
}

#[test]
fn agent_result_survives_query_mut_reassignment() {
    // Rust-specific: Public mutable Query access may replace the whole Query.
    let input = String::from("query-owned result");
    let mut agent = Agent::new();
    agent.set_query_str(&input);
    agent.set_key_from_query();
    *agent.query_mut() = Query::new();
    drop(input);
    assert_eq!(agent.key().as_str(), "query-owned result");
}

#[test]
fn agent_prefix_result_survives_query_setter_reassignment() {
    // Rust-specific: A stored result remains valid after changing the query.
    let input = String::from("prefix and suffix");
    let mut agent = Agent::new();
    agent.set_query_str(&input);
    agent.set_key_from_query_prefix(6);
    agent.set_query_str("replacement");
    drop(input);
    assert_eq!(agent.key().as_bytes(), b"prefix");
}

#[test]
fn agent_result_survives_state_buffer_replacement() {
    // Rust-specific: A public mutable State buffer may free its old allocation.
    let mut agent = Agent::new();
    agent.init_state().unwrap();
    agent
        .state_mut()
        .unwrap()
        .key_buf_mut()
        .extend_from_slice(b"state-owned result");
    agent.set_key_from_state_buf();
    *agent.state_mut().unwrap().key_buf_mut() = vec![b'x'; 4096];
    assert_eq!(agent.key().as_bytes(), b"state-owned result");
}

#[test]
fn agent_result_survives_whole_state_replacement() {
    // Rust-specific: state_mut permits replacing, not merely editing, State.
    let mut agent = Agent::new();
    agent.init_state().unwrap();
    agent
        .state_mut()
        .unwrap()
        .key_buf_mut()
        .extend_from_slice(b"whole state result");
    agent.set_key_from_state_buf();
    *agent.state_mut().unwrap() = Default::default();
    assert_eq!(agent.key().as_bytes(), b"whole state result");
}

#[test]
fn agent_cloned_lookup_result_survives_original_drop() {
    // Rust-specific: Agent Clone must own its Query and lookup result bytes.
    let trie = sample_trie();
    let cloned = {
        let input = String::from("apple");
        let mut original = Agent::new();
        original.set_query_str(&input);
        assert!(trie.lookup(&mut original));
        original.clone()
    };
    assert_eq!(cloned.query().as_str(), "apple");
    assert_eq!(cloned.key().as_str(), "apple");
}

#[test]
fn agent_cloned_reverse_result_survives_original_drop() {
    // Rust-specific: Reverse lookup uses ReadyToAll, not a predictive status.
    let trie = sample_trie();
    let cloned = {
        let mut original = Agent::new();
        original.set_query_str("apple");
        assert!(trie.lookup(&mut original));
        let id = original.key().id();
        original.set_query_id(id);
        trie.reverse_lookup(&mut original);
        original.clone()
    };
    assert_eq!(cloned.key().as_str(), "apple");
}

#[test]
fn agent_cloned_predictive_result_is_independent_of_both_states() {
    // Rust-specific: Neither cloned State nor original State owns cloned Key.
    let trie = sample_trie();
    let mut original = Agent::new();
    original.set_query_str("apple");
    assert!(trie.predictive_search(&mut original));
    let mut cloned = original.clone();
    *original.state_mut().unwrap().key_buf_mut() = Vec::new();
    *cloned.state_mut().unwrap().key_buf_mut() = Vec::new();
    drop(original);
    assert_eq!(cloned.key().as_str(), "apple");
}

#[test]
fn agent_clone_preserves_user_key_during_predictive_search() {
    // Rust-specific: Cloning cannot replace a user-assigned Key with State data.
    let trie = sample_trie();
    let mut original = Agent::new();
    original.set_query_str("apple");
    assert!(trie.predictive_search(&mut original));
    original.key_mut().set_str("user-assigned key");
    original.key_mut().set_id(43);
    let cloned = original.clone();
    assert_eq!(cloned.key().as_str(), "user-assigned key");
    assert_eq!(cloned.key().id(), 43);
}

#[test]
fn agent_result_survives_split_query_state_buffer_drop() {
    // Rust-specific: Borrow-splitting must not let State invalidate result bytes.
    let mut agent = Agent::new();
    agent.set_query_str("split query");
    agent.init_state().unwrap();
    agent
        .state_mut()
        .unwrap()
        .key_buf_mut()
        .extend_from_slice(b"split state result");
    agent.set_key_from_state_buf();
    let (query, state) = agent.query_bytes_and_state_mut();
    assert_eq!(query, b"split query");
    let old_buffer = std::mem::take(state.key_buf_mut());
    drop(old_buffer);
    assert_eq!(agent.key().as_bytes(), b"split state result");
}

#[test]
#[should_panic(expected = "Index out of bounds")]
fn key_get_rejects_large_usize_index_without_truncation() {
    // Rust-specific: Bounds checking cannot truncate an index before checking it.
    let mut key = Key::new();
    key.set_str("x");
    key.get(usize::MAX);
}

#[test]
#[cfg(target_pointer_width = "64")]
#[should_panic(expected = "Index out of bounds")]
fn key_get_rejects_index_that_truncates_to_zero() {
    // Rust-specific: The old u32 cast admitted this index before raw pointer use.
    let mut key = Key::new();
    key.set_str("x");
    key.get(1usize << 32);
}

#[test]
fn taken_query_survives_agent_drop() {
    // Rust-specific: query_mut also permits moving an entire Query out.
    let taken = {
        let input = String::from("taken query");
        let mut agent = Agent::new();
        agent.set_query_str(&input);
        std::mem::take(agent.query_mut())
    };
    assert_eq!(taken.as_str(), "taken query");
}

#[test]
fn taken_query_result_survives_agent_drop() {
    // Rust-specific: A private borrowed setter would still allow this escape.
    let taken = {
        let input = String::from("taken query result");
        let mut agent = Agent::new();
        agent.set_query_str(&input);
        agent.set_key_from_query();
        std::mem::take(agent.key_mut())
    };
    assert_eq!(taken.as_str(), "taken query result");
}

#[test]
fn taken_state_result_survives_agent_drop() {
    // Rust-specific: Moving a result out must sever any dependency on State.
    let taken = {
        let mut agent = Agent::new();
        agent.init_state().unwrap();
        agent
            .state_mut()
            .unwrap()
            .key_buf_mut()
            .extend_from_slice(b"taken state result");
        agent.set_key_from_state_buf();
        std::mem::take(agent.key_mut())
    };
    assert_eq!(taken.as_bytes(), b"taken state result");
}

#[test]
fn replaced_result_survives_agent_clear() {
    // Rust-specific: mem::replace can extract a Key just like mem::take.
    let mut agent = Agent::new();
    agent.init_state().unwrap();
    agent
        .state_mut()
        .unwrap()
        .key_buf_mut()
        .extend_from_slice(b"replaced result");
    agent.set_key_from_state_buf();
    let mut replacement = Key::new();
    replacement.set_str("new key");
    let escaped = std::mem::replace(agent.key_mut(), replacement);
    agent.clear();
    assert_eq!(escaped.as_bytes(), b"replaced result");
}

#[test]
fn swapped_result_survives_agent_clear() {
    // Rust-specific: Safe Key::swap must not leak a borrowed Agent buffer.
    let mut agent = Agent::new();
    agent.init_state().unwrap();
    agent
        .state_mut()
        .unwrap()
        .key_buf_mut()
        .extend_from_slice(b"swapped result");
    agent.set_key_from_state_buf();
    let mut escaped = Key::new();
    agent.key_mut().swap(&mut escaped);
    agent.clear();
    assert_eq!(escaped.as_bytes(), b"swapped result");
}

#[test]
fn cloned_keyset_key_survives_keyset_drop() {
    // Rust-specific: A cloned public Key cannot depend on its former Keyset.
    let cloned = {
        let mut keyset = Keyset::new();
        keyset.push_back_bytes(b"keyset clone", 2.5).unwrap();
        keyset.get(0).clone()
    };
    assert_eq!(cloned.as_bytes(), b"keyset clone");
    assert_eq!(cloned.weight(), 2.5);
}

#[test]
fn taken_keyset_key_survives_keyset_clear() {
    // Rust-specific: get_mut permits extracting a Key without calling Clone.
    let mut keyset = Keyset::new();
    keyset.push_back_str("keyset escape").unwrap();
    let escaped = std::mem::take(keyset.get_mut(0));
    keyset.clear();
    assert_eq!(escaped.as_str(), "keyset escape");
}

#[test]
fn cloned_keyset_key_survives_reset_reuse_and_clear() {
    // Rust-specific: reset/reuse must not repurpose storage held by a clone.
    let mut keyset = Keyset::new();
    keyset.push_back_str("before reset").unwrap();
    let cloned = keyset.get(0).clone();
    keyset.reset();
    keyset.push_back_str("after reset").unwrap();
    assert_eq!(keyset.size(), 1);
    assert_eq!(keyset.total_length(), "after reset".len());
    keyset.clear();
    assert_eq!(cloned.as_str(), "before reset");
}

#[test]
fn mutable_keyset_key_owns_reassigned_input() {
    // Rust-specific: Public mutable keys cannot bypass ownership via setters.
    let mut keyset = Keyset::new();
    keyset.push_back_str("original").unwrap();
    let input = String::from("replacement");
    keyset.get_mut(0).set_str(&input);
    drop(input);
    assert_eq!(keyset.get(0).as_str(), "replacement");
}

#[test]
fn keyset_push_back_key_preserves_owned_bytes_and_metadata() {
    // Rust-specific: Collection must not retain an input Key's byte storage.
    let mut keyset = Keyset::new();
    {
        let input = String::from("pushed key");
        let mut key = Key::new();
        key.set_str(&input);
        key.set_id(31);
        keyset.push_back_key(&key);
    }
    assert_eq!(keyset.get(0).as_str(), "pushed key");
    assert_eq!(keyset.get(0).id(), 31);
}

#[test]
fn marker_key_survives_mutable_keyset_escape() {
    // Rust-specific: Marker insertion must retain logical length and ownership.
    let escaped = {
        let mut keyset = Keyset::new();
        let mut key = Key::new();
        key.set_str("marked key");
        key.set_id(37);
        keyset.push_back_key_with_marker(&key, 0);
        std::mem::take(keyset.get_mut(0))
    };
    assert_eq!(escaped.as_bytes(), b"marked key");
    assert_eq!(escaped.length(), "marked key".len());
    assert_eq!(escaped.id(), 37);
}

#[test]
fn common_prefix_results_remain_owned_across_iteration_and_query_drop() {
    // Rust-specific: Taken and cloned prefix results must remain snapshots.
    let trie = sample_trie();
    let mut input = String::from("apple pie");
    let mut agent = Agent::new();
    agent.set_query_str(&input);
    let mut taken = Vec::new();
    let mut cloned = Vec::new();
    while trie.common_prefix_search(&mut agent) {
        cloned.push(agent.key().clone());
        taken.push(std::mem::take(agent.key_mut()));
    }
    assert!(!trie.common_prefix_search(&mut agent));
    input.clear();
    drop(input);
    agent.clear();
    let expected: &[&[u8]] = &[b"a", b"app", b"apple"];
    for results in [&taken, &cloned] {
        assert_eq!(results.len(), expected.len());
        for (result, expected) in results.iter().zip(expected) {
            assert_eq!(result.as_bytes(), *expected);
        }
    }
}

#[test]
fn predictive_results_remain_owned_across_iteration_and_state_drop() {
    // Rust-specific: Reused State storage cannot mutate previously yielded Keys.
    let trie = sample_trie();
    let mut agent = Agent::new();
    agent.set_query_str("app");
    let mut taken = Vec::new();
    let mut cloned = Vec::new();
    while trie.predictive_search(&mut agent) {
        cloned.push(agent.key().clone());
        taken.push(std::mem::take(agent.key_mut()));
    }
    assert!(!trie.predictive_search(&mut agent));
    agent.clear();
    let mut expected = vec![b"app".to_vec(), b"apple".to_vec(), b"apply".to_vec()];
    expected.sort();
    for results in [&taken, &cloned] {
        let mut actual: Vec<_> = results.iter().map(|key| key.as_bytes().to_vec()).collect();
        actual.sort();
        assert_eq!(actual, expected);
    }
}
