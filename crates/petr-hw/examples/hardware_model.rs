use std::ptr::NonNull;

use petr_core::TaggedNonNull;
use petr_hw::{MteAllocation, MteTag, PacModifier, SoftwarePac, SoftwarePacKey};

#[repr(align(16))]
struct Record(u64);

fn main() {
    let mut record = Record(42);
    let tagged = TaggedNonNull::<Record, 4>::new(NonNull::from(&mut record), 3).unwrap();

    let modifier = PacModifier::from_components(3, 1, 0);
    let pac = SoftwarePac::sign(tagged, modifier, SoftwarePacKey::from_seed(7));
    let _ = SoftwarePac::authenticate(pac, modifier, SoftwarePacKey::from_seed(7)).unwrap();

    let mte = MteAllocation::new(tagged, MteTag::new(9).unwrap());
    let _ = mte.validate(mte.issue()).unwrap();

    assert_eq!(record.0, 42);
}
