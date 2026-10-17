use petr_affinity::{CpuId, CpuSet, Placement};

#[test]
fn deterministic_fallback_never_selects_an_occupied_cpu() {
    let allowed = CpuSet::from_count(6).unwrap();
    for mask in 0u32..(1u32 << 6) {
        let occupied = (0..6)
            .filter(|cpu| (mask & (1u32 << cpu)) != 0)
            .map(CpuId)
            .collect::<Vec<_>>();

        for preferred in 0..6 {
            let resolved = Placement::Prefer(CpuId(preferred))
                .resolve(&allowed, &occupied)
                .unwrap();
            match resolved {
                Some(cpu) => {
                    assert!(allowed.contains(cpu));
                    assert!(!occupied.contains(&cpu));
                    if !occupied.contains(&CpuId(preferred)) {
                        assert_eq!(cpu, CpuId(preferred));
                    }
                }
                None => assert_eq!(occupied.len(), allowed.len()),
            }
        }
    }
}

#[test]
fn any_placement_uses_the_lowest_free_cpu() {
    let allowed = CpuSet::new(vec![CpuId(7), CpuId(3), CpuId(5)]).unwrap();
    assert_eq!(
        Placement::Any.resolve(&allowed, &[]).unwrap(),
        Some(CpuId(3))
    );
    assert_eq!(
        Placement::Any.resolve(&allowed, &[CpuId(3)]).unwrap(),
        Some(CpuId(5))
    );
    assert_eq!(
        Placement::Any
            .resolve(&allowed, &[CpuId(3), CpuId(5)])
            .unwrap(),
        Some(CpuId(7))
    );
    assert_eq!(
        Placement::Any
            .resolve(&allowed, &[CpuId(3), CpuId(5), CpuId(7)])
            .unwrap(),
        None
    );
}
