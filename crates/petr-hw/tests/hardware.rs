use std::ptr::NonNull;

use petr_core::{SplitTaggedNonNull, TaggedNonNull};
use petr_hw::{
    MteAllocation, MteError, MteHandle, MteTag, PacError, PacModifier, SoftwarePac, SoftwarePacKey,
    hardware_base, split_hardware_view,
};

#[repr(align(16))]
struct Aligned(u64);

#[test]
fn software_pac_binds_address_low_tag_and_modifier_without_rebuilding_pointer() {
    let mut value = Aligned(7);
    let ptr = TaggedNonNull::<Aligned, 4>::new(NonNull::from(&mut value), 5).unwrap();
    let key = SoftwarePacKey::from_seed(99);
    let modifier = PacModifier::from_components(3, 11, 2);
    let token = SoftwarePac::sign(ptr, modifier, key);

    let clean = SoftwarePac::authenticate(token, modifier, key).unwrap();
    assert_eq!(clean, NonNull::from(&mut value));
    assert_eq!(ptr.tag(), 5);
    assert_eq!(value.0, 7);

    assert_eq!(
        SoftwarePac::authenticate(token, PacModifier::new(modifier.0 ^ 1), key),
        Err(PacError::AuthenticationFailed),
    );
}

#[test]
fn mte_model_retagging_invalidates_stale_handles_even_when_tags_can_repeat() {
    let mut value = Aligned(9);
    let tagged = TaggedNonNull::<Aligned, 4>::new(NonNull::from(&mut value), 3).unwrap();
    let tag_a = MteTag::new(6).unwrap();
    let mut allocation = MteAllocation::new(tagged, tag_a);
    let stale = allocation.issue();
    assert!(allocation.validate(stale).is_ok());

    allocation.retag(MteTag::new(12).unwrap()).unwrap();
    assert_eq!(allocation.validate(stale), Err(MteError::ExpiredGeneration));

    allocation.retag(tag_a).unwrap();
    assert_eq!(allocation.validate(stale), Err(MteError::ExpiredGeneration));
}

#[test]
fn mte_model_distinguishes_wrong_pointer_tag_from_wrong_allocation() {
    let mut a = Aligned(1);
    let mut b = Aligned(2);
    let a_tagged = TaggedNonNull::<Aligned, 4>::new(NonNull::from(&mut a), 1).unwrap();
    let b_tagged = TaggedNonNull::<Aligned, 4>::new(NonNull::from(&mut b), 1).unwrap();
    let tag = MteTag::new(4).unwrap();
    let allocation = MteAllocation::new(a_tagged, tag);

    let wrong_tag = allocation.issue().with_pointer_tag(MteTag::new(5).unwrap());
    assert_eq!(allocation.validate(wrong_tag), Err(MteError::TagMismatch));

    let wrong_allocation = MteHandle::from_parts(b_tagged, tag, allocation.generation());
    assert_eq!(
        allocation.validate(wrong_allocation),
        Err(MteError::WrongAllocation)
    );
}

#[test]
fn software_top_byte_is_not_reused_as_hardware_metadata() {
    let mut value = Aligned(17);
    let split = SplitTaggedNonNull::<Aligned, 4>::new(NonNull::from(&mut value), 0xa7, 9).unwrap();
    let view = split_hardware_view(split);
    let base = hardware_base(split);

    assert_eq!(view.software_top, 0xa7);
    assert_eq!(view.low_tag, 9);
    assert_eq!(view.clean_addr, base.clean_addr());
    assert_eq!(base.tag(), 9);
    assert_eq!(base.clean(), NonNull::from(&mut value));
}

#[test]
fn mte_tags_are_exactly_four_bits() {
    assert_eq!(MteTag::new(15).unwrap().get(), 15);
    assert_eq!(MteTag::new(16), Err(MteError::InvalidTag));
}

#[test]
fn software_backend_advertises_both_protection_capabilities() {
    use petr_hw::{ProtectionBackend, ProtectionRequirement, backend_capabilities};

    let capabilities = backend_capabilities(ProtectionBackend::SoftwareModel);
    assert!(capabilities.supports(ProtectionRequirement::Authentication));
    assert!(capabilities.supports(ProtectionRequirement::MemoryTagging));
    assert!(capabilities.supports(ProtectionRequirement::AuthenticationAndMemoryTagging));
}
