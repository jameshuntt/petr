use petr_temporal::{Tick, Window};

#[test]
fn one_hot_windows_are_exact_for_every_supported_period_and_offset() {
    for period in 1u8..=64 {
        for hot in 0u8..period {
            let window = Window::new(period, 1u64 << hot, 0).unwrap();
            for tick in 0u64..u64::from(period) * 4 {
                assert_eq!(
                    window.eligible(Tick(tick)),
                    tick % u64::from(period) == u64::from(hot)
                );
            }
        }
    }
}

#[test]
fn phase_offsets_rotate_windows_without_changing_density() {
    for period in 1u8..=32 {
        let mask = if period == 1 {
            1
        } else {
            0b101u64 & ((1u64 << period) - 1)
        };
        let mask = if mask == 0 { 1 } else { mask };
        let baseline = Window::new(period, mask, 0).unwrap();
        let expected_per_cycle = (0..u64::from(period))
            .filter(|&t| baseline.eligible(Tick(t)))
            .count();

        for offset in 0u8..period {
            let shifted = Window::new(period, mask, offset).unwrap();
            let actual = (0..u64::from(period))
                .filter(|&t| shifted.eligible(Tick(t)))
                .count();
            assert_eq!(
                actual, expected_per_cycle,
                "period={period} offset={offset}"
            );
        }
    }
}
