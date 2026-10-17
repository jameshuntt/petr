use petr_affinity::*;

#[test]
fn cpu_set_is_sorted_and_unique() {
    let s = CpuSet::new(vec![CpuId(3), CpuId(1), CpuId(3)]).unwrap();
    assert_eq!(s.as_slice(), &[CpuId(1), CpuId(3)]);
}
#[test]
fn required_cpu_fails_if_busy() {
    let s = CpuSet::from_count(2).unwrap();
    assert_eq!(
        Placement::Require(CpuId(0)).resolve(&s, &[CpuId(0)]),
        Err(AffinityError::CpuBusy(CpuId(0)))
    );
}
#[test]
fn preferred_cpu_falls_back_deterministically() {
    let s = CpuSet::from_count(3).unwrap();
    assert_eq!(
        Placement::Prefer(CpuId(2))
            .resolve(&s, &[CpuId(2)])
            .unwrap(),
        Some(CpuId(0))
    );
}
