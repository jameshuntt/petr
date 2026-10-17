use petr_core::SelectorWord;
use petr_dispatch::{
    ChainProgram, ChainStep, DenseOpArg, DispatchError, OpFrame, OperationTable,
    SelectorPermutation, ShuffledOperationTable, default_dense_table, default_table, dense_acc_add,
    dense_acc_mul, dense_add_prefix, dense_add_suffix, table_from_toml,
};

#[test]
fn default_table_executes_registered_builtin_selectors_exactly() {
    let table = default_table();

    assert_eq!(
        table.execute(0x00, OpFrame::text("Data")).unwrap(),
        OpFrame::text("Data")
    );
    assert_eq!(
        table.execute(0x01, OpFrame::text("Data")).unwrap(),
        OpFrame::text("Prefix_Data")
    );
    assert_eq!(
        table.execute(0x02, OpFrame::text("Data")).unwrap(),
        OpFrame::text("Data_Suffix")
    );
    assert_eq!(
        table.execute(0x10, OpFrame::text("17")).unwrap(),
        OpFrame::text("36")
    );
    assert_eq!(
        table.execute(0x11, OpFrame::text("17")).unwrap(),
        OpFrame::text("20")
    );
    assert_eq!(
        table.execute(0x20, OpFrame::text("ff")).unwrap(),
        OpFrame::text("FF")
    );
    assert_eq!(
        table.execute(0x21, OpFrame::text("FF")).unwrap(),
        OpFrame::text("100")
    );
    assert_eq!(
        table.execute(0x30, OpFrame::acc(40)).unwrap(),
        OpFrame::acc(41)
    );
    assert_eq!(
        table.execute(0x31, OpFrame::acc(40)).unwrap(),
        OpFrame::acc(80)
    );
}

#[test]
fn sparse_operation_table_reports_missing_selector_and_contains_is_exact() {
    let table = default_table();
    assert!(table.contains(0x20));
    assert!(!table.contains(0x99));
    assert_eq!(
        table.execute(0x99, OpFrame::text("Data")).unwrap_err(),
        DispatchError::OperationNotFound(0x99)
    );
}

#[test]
fn execute_word_uses_only_the_function_byte_of_4_4_8_selector() {
    let table = default_table();
    let selector = SelectorWord::new(0x0F, 0x0E, 0x20);
    assert_eq!(
        table.execute_word(selector, OpFrame::text("2a")).unwrap(),
        OpFrame::text("2A")
    );
}

#[test]
fn execute_or_nop_preserves_frame_for_missing_selectors_and_errors_to_op_error() {
    let table = default_table();
    assert_eq!(
        table.execute_or_nop(0xFE, OpFrame::text("Keep")),
        OpFrame::text("Keep")
    );
    assert_eq!(
        table.execute_or_nop(0x20, OpFrame::text("not hex")),
        OpFrame::text("<op-error>")
    );
}

#[test]
fn custom_table_registration_overwrites_selector_deterministically() {
    let mut table = OperationTable::new();
    table.register(
        0x30,
        "add",
        petr_dispatch::op_acc_add,
        petr_dispatch::OpArg::n(5),
    );
    assert_eq!(
        table.execute(0x30, OpFrame::acc(1)).unwrap(),
        OpFrame::acc(6)
    );

    table.register(
        0x30,
        "mul",
        petr_dispatch::op_acc_mul,
        petr_dispatch::OpArg::n(7),
    );
    assert_eq!(
        table.execute(0x30, OpFrame::acc(6)).unwrap(),
        OpFrame::acc(42)
    );
}

#[test]
fn dense_table_has_nop_for_all_unregistered_slots_and_no_missing_entry_errors() {
    let table = default_dense_table();

    for selector in 0u8..=255 {
        let input = if selector == 0x20 {
            OpFrame::text("ff")
        } else {
            OpFrame::acc(selector as i64)
        };
        let out = table.execute(selector, input.clone()).unwrap();
        match selector {
            0x01 => assert_eq!(out.text, "Prefix_"),
            0x02 => assert_eq!(out.text, "_Suffix"),
            0x20 => assert_eq!(out, OpFrame::text("FF")),
            0x30 => assert_eq!(out.acc, selector as i64 + 1),
            0x31 => assert_eq!(out.acc, selector as i64 * 2),
            _ => assert_eq!(out, input),
        }
    }
}

#[test]
fn dense_table_static_registration_is_exact() {
    let mut table = petr_dispatch::DenseOperationTable::new();
    table.register(0x01, "prefix", dense_add_prefix, DenseOpArg::text("A"));
    table.register(0x02, "suffix", dense_add_suffix, DenseOpArg::text("Z"));
    table.register(0x30, "add", dense_acc_add, DenseOpArg::n(10));
    table.register(0x31, "mul", dense_acc_mul, DenseOpArg::n(10));

    assert_eq!(table.name(0x01), "prefix");
    assert_eq!(
        table.execute(0x01, OpFrame::text("x")).unwrap(),
        OpFrame::text("Ax")
    );
    assert_eq!(
        table.execute(0x02, OpFrame::text("x")).unwrap(),
        OpFrame::text("xZ")
    );
    assert_eq!(
        table.execute(0x30, OpFrame::acc(3)).unwrap(),
        OpFrame::acc(13)
    );
    assert_eq!(
        table.execute(0x31, OpFrame::acc(3)).unwrap(),
        OpFrame::acc(30)
    );
    assert_eq!(
        table.execute(0x80, OpFrame::text("nop")).unwrap(),
        OpFrame::text("nop")
    );
}

#[test]
fn selector_permutation_is_a_full_bijection_for_many_seeds() {
    for seed in [0, 1, 2, 0xCAFE_BABE, 0xDEAD_BEEF_DEAD_BEEF] {
        let perm = SelectorPermutation::from_seed(seed);
        let mut seen = [false; 256];

        for logical in 0u8..=255 {
            let physical = perm.encode(logical);
            assert!(
                !seen[physical as usize],
                "duplicate physical selector {physical:#04x} for seed {seed:#x}"
            );
            seen[physical as usize] = true;
            assert_eq!(perm.decode(physical), logical);
        }

        assert!(seen.iter().all(|v| *v));
    }
}

#[test]
fn shuffled_table_executes_physical_selectors_as_logical_operations() {
    let logical = default_table();
    let shuffled = ShuffledOperationTable::new(logical, 0xFACE_FEED);
    let physical = shuffled.encode_selector(0x20);

    assert_eq!(
        shuffled
            .execute_physical(physical, OpFrame::text("2a"))
            .unwrap(),
        OpFrame::text("2A")
    );
}

#[test]
fn toml_config_builds_table_and_reports_bad_configs() {
    let toml = r#"
[operations]
"0x01" = { type = "add_prefix", prefix = "pre_" }
"0x02" = { type = "add_suffix", suffix = "_post" }
"0x20" = { type = "octet_add", value = 16 }
"#;
    let table = table_from_toml(toml).unwrap();
    assert_eq!(
        table.execute(0x01, OpFrame::text("x")).unwrap(),
        OpFrame::text("pre_x")
    );
    assert_eq!(
        table.execute(0x02, OpFrame::text("x")).unwrap(),
        OpFrame::text("x_post")
    );
    assert_eq!(
        table.execute(0x20, OpFrame::text("FF")).unwrap(),
        OpFrame::text("10F")
    );

    let missing = table_from_toml(
        r#"[operations]
"0x01" = { type = "add_prefix" }
"#,
    )
    .unwrap_err();
    assert_eq!(
        missing,
        DispatchError::MissingParam {
            op: "add_prefix".into(),
            param: "prefix"
        }
    );

    let unknown = table_from_toml(
        r#"[operations]
"0x01" = { type = "does_not_exist" }
"#,
    )
    .unwrap_err();
    assert_eq!(
        unknown,
        DispatchError::UnknownOperationType("does_not_exist".into())
    );
}

#[test]
fn toml_selector_parser_accepts_hex_and_decimal_and_rejects_overflow() {
    assert_eq!(petr_dispatch::parse_selector("0x20").unwrap(), 0x20);
    assert_eq!(petr_dispatch::parse_selector("0Xff").unwrap(), 0xFF);
    assert_eq!(petr_dispatch::parse_selector("32").unwrap(), 32);
    assert!(matches!(
        petr_dispatch::parse_selector("256"),
        Err(DispatchError::InvalidInput(_))
    ));
    assert!(matches!(
        petr_dispatch::parse_selector("not-a-selector"),
        Err(DispatchError::InvalidInput(_))
    ));
}

#[test]
fn chain_program_runs_tail_style_selector_sequence_and_detects_bad_pc() {
    let table = default_table();
    let mut chain = ChainProgram::new();
    let a = chain.push(ChainStep {
        selector: SelectorWord::new(1, 1, 0x20),
        arg: Default::default(),
        next: None,
    });
    let b = chain.push(ChainStep {
        selector: SelectorWord::new(1, 1, 0x01),
        arg: Default::default(),
        next: None,
    });
    let c = chain.push(ChainStep {
        selector: SelectorWord::new(1, 1, 0x02),
        arg: Default::default(),
        next: None,
    });

    // Rebuild in executable order: encode hex first, then decorate.
    let mut ordered = ChainProgram::new();
    let s0 = ordered.push(ChainStep {
        selector: SelectorWord::new(1, 1, 0x20),
        arg: Default::default(),
        next: Some(1),
    });
    let _s1 = ordered.push(ChainStep {
        selector: SelectorWord::new(1, 1, 0x01),
        arg: Default::default(),
        next: Some(2),
    });
    let _s2 = ordered.push(ChainStep {
        selector: SelectorWord::new(1, 1, 0x02),
        arg: Default::default(),
        next: None,
    });

    assert_eq!(
        ordered.run(&table, s0, OpFrame::text("ff")).unwrap(),
        OpFrame::text("Prefix_FF_Suffix")
    );
    assert_eq!(
        chain.run(&table, 99, OpFrame::text("x")).unwrap_err(),
        DispatchError::InvalidContinuation(99)
    );
    assert_eq!(a, 0);
    assert_eq!(b, 1);
    assert_eq!(c, 2);
}

#[test]
fn chain_program_guard_catches_cycles() {
    let table = default_table();
    let mut chain = ChainProgram::new();
    let start = chain.push(ChainStep {
        selector: SelectorWord::new(1, 1, 0x00),
        arg: Default::default(),
        next: Some(0),
    });
    let err = chain.run(&table, start, OpFrame::text("loop")).unwrap_err();
    assert!(matches!(err, DispatchError::InvalidInput(msg) if msg.contains("cycle")));
}
