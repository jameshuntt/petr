#![cfg(loom)]

use loom::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use loom::thread;
use petr_affinity::{CpuSet, Placement};
use petr_core::Key;
use petr_execution::*;
use petr_resource::{ResourceManager, ResourcePool};
use petr_temporal::{Tick, Window};

fn plan() -> ExecutionPlan {
    let mut plan = ExecutionPlan::new(CpuSet::from_count(1).unwrap());
    plan.insert(HandlerSpec {
        id: 1,
        name: "loom",
        window: Window::new(1, 1, 0).unwrap(),
        placement: Placement::Any,
        obligation: Obligation::Required,
        resource: None,
    })
    .unwrap();
    plan
}

fn wait_until(flag: &AtomicBool) {
    while !flag.load(Ordering::Acquire) {
        thread::yield_now();
    }
}

#[test]
fn completed_authority_is_published_to_a_concurrent_clone() {
    loom::model(|| {
        let plan = plan();
        let mut resources =
            ResourceManager::<4>::with_key(90, ResourcePool::new(0, 0), Key::<4>::from_seed(1));
        let mut admitted = plan
            .admit_in_epoch(&mut resources, Tick(0), AdmissionEpoch::new(1))
            .unwrap();
        let authority = admitted.lease(1).unwrap().authority();

        let published = Arc::new(AtomicBool::new(false));
        let observer_flag = Arc::clone(&published);
        let observer_authority = authority.clone();
        let observer = thread::spawn(move || {
            wait_until(&observer_flag);
            assert_eq!(
                observer_authority.status(),
                ExecutionAuthorityStatus::Completed
            );
            assert!(!observer_authority.is_active());
        });

        admitted.complete_handler(1).unwrap();
        published.store(true, Ordering::Release);
        observer.join().unwrap();
        assert_eq!(authority.status(), ExecutionAuthorityStatus::Completed);
    });
}

#[test]
fn drop_revocation_is_published_and_never_revives() {
    loom::model(|| {
        let plan = plan();
        let mut resources =
            ResourceManager::<4>::with_key(91, ResourcePool::new(0, 0), Key::<4>::from_seed(2));
        let admitted = plan
            .admit_in_epoch(&mut resources, Tick(0), AdmissionEpoch::new(2))
            .unwrap();
        let authority = admitted.lease(1).unwrap().authority();

        let published = Arc::new(AtomicBool::new(false));
        let observer_flag = Arc::clone(&published);
        let observer_authority = authority.clone();
        let observer = thread::spawn(move || {
            wait_until(&observer_flag);
            assert_eq!(
                observer_authority.status(),
                ExecutionAuthorityStatus::Revoked
            );
            assert!(!observer_authority.is_active());
        });

        drop(admitted);
        published.store(true, Ordering::Release);
        observer.join().unwrap();
        assert_eq!(authority.status(), ExecutionAuthorityStatus::Revoked);
    });
}

#[test]
fn drop_cannot_overwrite_an_existing_terminal_reason() {
    loom::model(|| {
        let plan = plan();
        let mut resources =
            ResourceManager::<4>::with_key(92, ResourcePool::new(0, 0), Key::<4>::from_seed(3));
        let mut admitted = plan
            .admit_in_epoch(&mut resources, Tick(0), AdmissionEpoch::new(3))
            .unwrap();
        let authority = admitted.lease(1).unwrap().authority();
        admitted.cancel_handler(1).unwrap();
        drop(admitted);
        assert_eq!(authority.status(), ExecutionAuthorityStatus::Cancelled);
    });
}
