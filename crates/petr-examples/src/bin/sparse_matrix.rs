use petr_math::SparseMatrix;

fn main() {
    let m = SparseMatrix::demo();
    let y = m.mul_vec_prophecy(&[10.0, 20.0, 30.0]);
    println!("y = {:?}", y);
}
