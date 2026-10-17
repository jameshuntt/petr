use petr_protection::{ProtectionCapabilities, ProtectionError, ProtectionRequirement, require};

#[test]
fn requirements_report_exact_capability_needs() {
    assert!(!ProtectionRequirement::None.requires_authentication());
    assert!(!ProtectionRequirement::None.requires_memory_tagging());
    assert!(ProtectionRequirement::Authentication.requires_authentication());
    assert!(!ProtectionRequirement::Authentication.requires_memory_tagging());
    assert!(!ProtectionRequirement::MemoryTagging.requires_authentication());
    assert!(ProtectionRequirement::MemoryTagging.requires_memory_tagging());
    assert!(ProtectionRequirement::AuthenticationAndMemoryTagging.requires_authentication());
    assert!(ProtectionRequirement::AuthenticationAndMemoryTagging.requires_memory_tagging());
}

#[test]
fn capability_negotiation_is_compositional() {
    let auth = ProtectionCapabilities::AUTHENTICATION;
    let mte = ProtectionCapabilities::MEMORY_TAGGING;
    let both = auth.union(mte);

    assert!(both.supports(ProtectionRequirement::AuthenticationAndMemoryTagging));
    assert_eq!(both.intersection(auth), auth);
    assert!(!auth.supports(ProtectionRequirement::MemoryTagging));
}

#[test]
fn unsupported_requirements_are_explicit_errors() {
    assert_eq!(
        require(
            ProtectionCapabilities::AUTHENTICATION,
            ProtectionRequirement::MemoryTagging
        ),
        Err(ProtectionError::Unsupported {
            requirement: ProtectionRequirement::MemoryTagging,
            capabilities: ProtectionCapabilities::AUTHENTICATION,
        }),
    );
}
