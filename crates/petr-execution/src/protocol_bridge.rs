//! Optional policy bridge to `waker_protocol`.
//!
//! PETR does not own protocol task memory or wake delivery. It only translates
//! an admitted handler's QoS policy into the existing protocol scheduler policy.

use crate::ExecutionProfile;

pub fn protocol_qos(qos: scope::QoSClass) -> waker_protocol::QoSClass {
    match qos {
        scope::QoSClass::Realtime => waker_protocol::QoSClass::Realtime,
        scope::QoSClass::Interactive => waker_protocol::QoSClass::Interactive,
        scope::QoSClass::Standard => waker_protocol::QoSClass::Standard,
        scope::QoSClass::Batch => waker_protocol::QoSClass::Batch,
        scope::QoSClass::Background => waker_protocol::QoSClass::Background,
    }
}

pub fn task_policy(profile: ExecutionProfile) -> waker_protocol::TaskPolicy {
    waker_protocol::TaskPolicy {
        qos: protocol_qos(profile.qos),
        ..waker_protocol::TaskPolicy::default()
    }
}
