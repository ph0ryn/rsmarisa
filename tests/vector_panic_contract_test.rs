//! Rust-specific regressions for documented vector panic contracts in all builds.

use rsmarisa::grimoire::vector::bit_vector::BitVector;
use rsmarisa::grimoire::vector::flat_vector::FlatVector;
use rsmarisa::grimoire::vector::vector::Vector;
use std::panic::{catch_unwind, UnwindSafe};

fn assert_panics_with(expected: &str, operation: impl FnOnce() + UnwindSafe) {
    let panic = catch_unwind(operation).expect_err("operation must panic");
    let message = panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .expect("panic must contain a string message");
    assert_eq!(message, expected);
}

#[test]
fn select0_rejects_missing_or_disabled_index() {
    let mut bits = BitVector::new();
    bits.push_back(false);
    assert_panics_with("Select0 index not built", || {
        bits.select0(0);
    });

    bits.build(false, false);
    assert_panics_with("Select0 index not built", || {
        bits.select0(0);
    });

    let mut indexed_bits = BitVector::new();
    indexed_bits.push_back(false);
    indexed_bits.build(true, true);
    assert_eq!(indexed_bits.select0(0), 0);
    indexed_bits.disable_select0();
    assert_panics_with("Select0 index not built", || {
        indexed_bits.select0(0);
    });
}

#[test]
fn select1_rejects_missing_or_disabled_index() {
    let mut bits = BitVector::new();
    bits.push_back(true);
    assert_panics_with("Select1 index not built", || {
        bits.select1(0);
    });

    bits.build(false, false);
    assert_panics_with("Select1 index not built", || {
        bits.select1(0);
    });

    let mut indexed_bits = BitVector::new();
    indexed_bits.push_back(true);
    indexed_bits.build(true, true);
    assert_eq!(indexed_bits.select1(0), 0);
    indexed_bits.disable_select1();
    assert_panics_with("Select1 index not built", || {
        indexed_bits.select1(0);
    });
}

#[test]
fn select0_rejects_out_of_range_rank() {
    for size in [0, 1, 63, 64, 65, 511, 512, 513, 1024, 1025] {
        let mut bits = BitVector::new();
        for i in 0..size {
            bits.push_back(i % 2 != 0);
        }
        bits.build(true, true);

        for rank in [bits.num_0s(), bits.num_0s() + 1, usize::MAX] {
            assert_panics_with("Index out of bounds", || {
                bits.select0(rank);
            });
        }
    }
}

#[test]
fn select1_rejects_out_of_range_rank() {
    for size in [0, 1, 63, 64, 65, 511, 512, 513, 1024, 1025] {
        let mut bits = BitVector::new();
        for i in 0..size {
            bits.push_back(i % 2 == 0);
        }
        bits.build(true, true);

        for rank in [bits.num_1s(), bits.num_1s() + 1, usize::MAX] {
            assert_panics_with("Index out of bounds", || {
                bits.select1(rank);
            });
        }
    }
}

#[test]
fn select_preserves_valid_word_and_index_boundaries() {
    for size in [0, 1, 63, 64, 65, 511, 512, 513, 1024, 1025, 2050] {
        let mut bits = BitVector::new();
        for i in 0..size {
            bits.push_back(i % 2 != 0);
        }
        bits.build(true, true);

        for rank in 0..bits.num_0s() {
            assert_eq!(bits.select0(rank), rank * 2);
        }
        for rank in 0..bits.num_1s() {
            assert_eq!(bits.select1(rank), rank * 2 + 1);
        }
    }
}

#[test]
fn flat_vector_rejects_logical_out_of_bounds() {
    let cases: &[&[u32]] = &[
        &[],
        &[0, 0, 0],
        &[1],
        &[u32::MAX, u32::MAX],
        &[0, 1, 2, 3, 4],
    ];
    for &case in cases {
        let mut values = Vector::new();
        for &value in case {
            values.push_back(value);
        }
        let mut packed = FlatVector::new();
        packed.build(&values);

        for (i, &value) in case.iter().enumerate() {
            assert_eq!(packed.get(i), value);
        }
        for index in [packed.size(), packed.size() + 1, usize::MAX] {
            assert_panics_with("Index out of bounds", || {
                packed.get(index);
            });
        }
    }
}

#[test]
fn flat_vector_preserves_valid_packed_values() {
    for width in [0, 1, 3, 7, 16, 31, 32] {
        let mask = ((1u64 << width) - 1) as u32;
        let mut values = Vector::new();
        values.push_back(mask);
        for i in 1..65u32 {
            values.push_back(i.wrapping_mul(1_234_567) & mask);
        }
        let mut packed = FlatVector::new();
        packed.build(&values);

        assert_eq!(packed.value_size(), width);
        for i in 0..values.size() {
            assert_eq!(packed.get(i), values[i]);
        }
    }
}
