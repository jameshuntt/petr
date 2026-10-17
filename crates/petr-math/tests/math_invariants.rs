use petr_core::{PortHint, ProphecyBits};
use petr_math::{SparseCell, SparseMatrix};

fn active() -> ProphecyBits {
    ProphecyBits {
        maybe_zero: false,
        maybe_empty: false,
        hot: true,
        skip_hint: false,
        port_hint: PortHint::Vector,
    }
}

fn skip() -> ProphecyBits {
    ProphecyBits {
        maybe_zero: true,
        maybe_empty: false,
        hot: false,
        skip_hint: true,
        port_hint: PortHint::Vector,
    }
}

#[test]
fn demo_sparse_matrix_matches_expected_vector_result() {
    let m = SparseMatrix::demo();
    assert_eq!(
        m.mul_vec_prophecy(&[10.0, 20.0, 30.0]),
        vec![20.0, 80.0, 240.0]
    );
}

#[test]
fn active_cells_match_naive_sparse_multiply_and_skip_cells_do_not_contribute() {
    let matrix = SparseMatrix {
        rows: 3,
        cols: 4,
        cells: vec![
            SparseCell {
                row: 0,
                col: 0,
                value: 2.0,
                prophecy: active(),
            },
            SparseCell {
                row: 0,
                col: 1,
                value: 9999.0,
                prophecy: skip(),
            },
            SparseCell {
                row: 1,
                col: 2,
                value: -3.0,
                prophecy: active(),
            },
            SparseCell {
                row: 2,
                col: 3,
                value: 0.5,
                prophecy: active(),
            },
        ],
    };

    let x = [4.0, 5.0, 6.0, 8.0];
    let y = matrix.mul_vec_prophecy(&x);
    assert_eq!(y, vec![8.0, -18.0, 4.0]);
}

#[test]
fn out_of_bounds_active_cells_are_guarded_and_do_not_panic() {
    let matrix = SparseMatrix {
        rows: 2,
        cols: 2,
        cells: vec![
            SparseCell {
                row: 99,
                col: 0,
                value: 5.0,
                prophecy: active(),
            },
            SparseCell {
                row: 0,
                col: 99,
                value: 5.0,
                prophecy: active(),
            },
            SparseCell {
                row: 1,
                col: 1,
                value: 7.0,
                prophecy: active(),
            },
        ],
    };

    assert_eq!(matrix.mul_vec_prophecy(&[2.0, 3.0]), vec![0.0, 21.0]);
}
