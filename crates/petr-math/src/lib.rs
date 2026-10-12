//! Matrix/math demos using selector-style active/nop dispatch.

use petr_core::{PortHint, ProphecyBits};

/// Sparse matrix cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparseCell {
    /// Row index.
    pub row: usize,
    /// Column index.
    pub col: usize,
    /// Cell value.
    pub value: f64,
    /// Prophecy bits: zero/skip cells can use NOP dispatch.
    pub prophecy: ProphecyBits,
}

/// Sparse matrix in coordinate form.
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    /// Number of rows.
    pub rows: usize,
    /// Number of columns.
    pub cols: usize,
    /// Cells, including possible zero/skip cells for demo purposes.
    pub cells: Vec<SparseCell>,
}

type CellOp = fn(&SparseCell, &[f64], &mut [f64]);

fn cell_nop(_cell: &SparseCell, _x: &[f64], _y: &mut [f64]) {}

fn cell_mul(cell: &SparseCell, x: &[f64], y: &mut [f64]) {
    if cell.col < x.len() && cell.row < y.len() {
        y[cell.row] += cell.value * x[cell.col];
    }
}

static CELL_TABLE: [CellOp; 2] = [cell_mul, cell_nop];

impl SparseMatrix {
    /// Multiply matrix by vector using prophecy-directed active/nop dispatch.
    pub fn mul_vec_prophecy(&self, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; self.rows];
        for cell in &self.cells {
            let idx = cell.prophecy.skip_hint as usize;
            CELL_TABLE[idx](cell, x, &mut y);
        }
        y
    }

    /// Build a demo matrix.
    pub fn demo() -> Self {
        let active = ProphecyBits { maybe_zero: false, maybe_empty: false, hot: true, skip_hint: false, port_hint: PortHint::Vector };
        let zero = ProphecyBits { maybe_zero: true, maybe_empty: false, hot: false, skip_hint: true, port_hint: PortHint::Vector };
        Self {
            rows: 3,
            cols: 3,
            cells: vec![
                SparseCell { row: 0, col: 0, value: 2.0, prophecy: active },
                SparseCell { row: 0, col: 1, value: 0.0, prophecy: zero },
                SparseCell { row: 1, col: 1, value: 4.0, prophecy: active },
                SparseCell { row: 2, col: 2, value: 8.0, prophecy: active },
            ],
        }
    }
}
