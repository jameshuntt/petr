use std::collections::HashMap;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use petr_dispatch::{DenseOpArg, DenseOperationTable, OpFrame, OperationTable, default_table};

fn match_4(selector: u8, mut frame: OpFrame) -> OpFrame {
    match selector {
        0x30 => frame.acc += 1,
        0x31 => frame.acc *= 2,
        0x32 => frame.acc -= 3,
        0x33 => frame.acc ^= 0x55,
        _ => {}
    }
    frame
}

fn match_32(selector: u8, mut frame: OpFrame) -> OpFrame {
    match selector {
        0x00..=0x1F => frame.acc += selector as i64,
        0x30 => frame.acc += 1,
        0x31 => frame.acc *= 2,
        _ => {}
    }
    frame
}

fn match_128(selector: u8, mut frame: OpFrame) -> OpFrame {
    match selector {
        0x00..=0x7F => frame.acc = frame.acc.wrapping_add((selector as i64) ^ 0x2A),
        0x80..=0xBF => frame.acc = frame.acc.wrapping_sub(selector as i64),
        _ => {}
    }
    frame
}

fn hash_add(mut frame: OpFrame) -> OpFrame {
    frame.acc += 1;
    frame
}

fn hash_mul(mut frame: OpFrame) -> OpFrame {
    frame.acc *= 2;
    frame
}

fn build_hash_table() -> HashMap<u8, fn(OpFrame) -> OpFrame> {
    let mut map = HashMap::new();
    map.insert(0x30, hash_add as fn(OpFrame) -> OpFrame);
    map.insert(0x31, hash_mul as fn(OpFrame) -> OpFrame);
    map
}

fn build_acc_dense() -> DenseOperationTable {
    let mut table = DenseOperationTable::new();
    table.register(
        0x32,
        "acc_sub3",
        |mut frame, _arg| {
            frame.acc -= 3;
            Ok(frame)
        },
        DenseOpArg::ZERO,
    );
    table.register(
        0x33,
        "acc_xor",
        |mut frame, _arg| {
            frame.acc ^= 0x55;
            Ok(frame)
        },
        DenseOpArg::ZERO,
    );
    table
}

fn bench_batch_match(selectors: &[u8], mut frame: OpFrame, mode: u8) -> OpFrame {
    for selector in selectors {
        frame = match mode {
            4 => match_4(black_box(*selector), black_box(frame)),
            32 => match_32(black_box(*selector), black_box(frame)),
            _ => match_128(black_box(*selector), black_box(frame)),
        };
    }
    frame
}

fn bench_batch_option(table: &OperationTable, selectors: &[u8], mut frame: OpFrame) -> OpFrame {
    for selector in selectors {
        frame = table.execute_or_nop(black_box(*selector), black_box(frame));
    }
    frame
}

fn bench_batch_dense(table: &DenseOperationTable, selectors: &[u8], mut frame: OpFrame) -> OpFrame {
    for selector in selectors {
        frame = table
            .execute(black_box(*selector), black_box(frame))
            .unwrap();
    }
    frame
}

fn bench_batch_hash(
    map: &HashMap<u8, fn(OpFrame) -> OpFrame>,
    selectors: &[u8],
    mut frame: OpFrame,
) -> OpFrame {
    for selector in selectors {
        if let Some(func) = map.get(selector) {
            frame = func(black_box(frame));
        }
    }
    frame
}

fn dispatch_bench(c: &mut Criterion) {
    let dynamic = default_table();
    let dense = build_acc_dense();
    let hash = build_hash_table();
    let tiny = [0x30u8, 0x31, 0x32, 0x33, 0x7F];
    let medium: Vec<u8> = (0..128).map(|i| (i % 48) as u8).collect();
    let large: Vec<u8> = (0..4096).map(|i| (i % 192) as u8).collect();

    c.bench_function("match_4_tiny", |b| {
        b.iter(|| black_box(bench_batch_match(&tiny, OpFrame::acc(1), 4)))
    });

    c.bench_function("match_32_medium", |b| {
        b.iter(|| black_box(bench_batch_match(&medium, OpFrame::acc(1), 32)))
    });

    c.bench_function("match_128_large", |b| {
        b.iter(|| black_box(bench_batch_match(&large, OpFrame::acc(1), 128)))
    });

    c.bench_function("hash_dispatch_tiny", |b| {
        b.iter(|| black_box(bench_batch_hash(&hash, &tiny, OpFrame::acc(1))))
    });

    c.bench_function("option_table_tiny", |b| {
        b.iter(|| black_box(bench_batch_option(&dynamic, &tiny, OpFrame::acc(1))))
    });

    c.bench_function("dense_table_tiny", |b| {
        b.iter(|| black_box(bench_batch_dense(&dense, &tiny, OpFrame::acc(1))))
    });

    c.bench_function("dense_table_large", |b| {
        b.iter(|| black_box(bench_batch_dense(&dense, &large, OpFrame::acc(1))))
    });
}

criterion_group!(benches, dispatch_bench);
criterion_main!(benches);
