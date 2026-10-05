//! Device lease state, shared by admission and native lifecycle callbacks.
use crate::RECEIPT_CAPACITY;
use gpui_storybook_automation::StorybookAutomationError;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(crate) struct Receipts {
    pub(crate) ids: BTreeSet<u64>,
    order: VecDeque<u64>,
}
impl Receipts {
    pub(crate) fn insert(&mut self, id: u64) {
        self.ids.insert(id);
        self.order.push_back(id);
        if self.order.len() > RECEIPT_CAPACITY {
            self.ids
                .remove(&self.order.pop_front().expect("full receipt window"));
        }
    }
}

#[derive(Default)]
pub(crate) struct GateState {
    pub(crate) active: Option<u64>,
    next: u64,
    seed: String,
    generation: u64,
    pub(crate) suspended: bool,
    pub(crate) session: String,
    pub(crate) receipts: Receipts,
}

impl GateState {
    pub(crate) fn new(seed: String) -> Self {
        Self {
            session: seed.clone(),
            seed,
            ..Default::default()
        }
    }
    pub(crate) fn rotate(&mut self) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("session generations exhausted");
        self.session = format!("{}.{}", self.seed, self.generation);
        self.receipts = Receipts::default();
        self.active = None;
        self.suspended = false;
    }
    pub(crate) fn release(&mut self, token: u64) {
        if self.active == Some(token) {
            self.active = None;
        }
    }
    pub(crate) fn suspend(&mut self) {
        self.suspended = true;
        self.active = None;
    }
    pub(crate) fn acquire_token(&mut self) -> Result<u64, StorybookAutomationError> {
        if self.suspended {
            return Err(StorybookAutomationError::NoLiveHost);
        }
        if self.active.is_some() {
            return Err(StorybookAutomationError::AutomationBusy);
        }
        self.next = self.next.checked_add(1).expect("operation IDs exhausted");
        let token = self.next;
        self.active = Some(token);
        Ok(token)
    }
}

#[derive(Clone, Default)]
pub struct OperationGate(pub(crate) Arc<Mutex<GateState>>);
impl OperationGate {
    pub(crate) fn lease(&self, token: u64) -> MutationLease {
        MutationLease {
            gate: self.clone(),
            token,
        }
    }
    pub fn acquire(&self) -> Result<MutationLease, StorybookAutomationError> {
        let mut state = self.0.lock().expect("operation gate");
        let token = state.acquire_token()?;
        Ok(MutationLease {
            gate: self.clone(),
            token,
        })
    }
    pub fn invalidate(&self) {
        self.0.lock().expect("operation gate").active = None;
    }
    /// Revoke work and stop admission immediately on the native surface owner
    /// thread. Session replacement reopens it after GPUI attachment invalidation.
    pub fn suspend(&self) {
        let mut state = self.0.lock().expect("operation gate");
        state.suspend();
    }
    pub fn busy(&self) -> bool {
        self.0.lock().expect("operation gate").active.is_some()
    }
}

pub struct MutationLease {
    gate: OperationGate,
    pub(crate) token: u64,
}
/// A non-owning permit for work queued on a native owner thread. Cloning it
/// never extends or releases ownership. The callback checks it immediately
/// before dispatch, serialized with that thread's surface lifecycle callbacks.
#[derive(Clone)]
pub struct MutationPermit {
    gate: OperationGate,
    pub(crate) token: u64,
}
impl MutationPermit {
    pub fn is_current(&self) -> bool {
        self.gate.0.lock().expect("operation gate").active == Some(self.token)
    }
}
impl MutationLease {
    /// Whether this lease still owns the device operation. Explicit host
    /// invalidation revokes queued work; callers check before native dispatch.
    pub fn is_current(&self) -> bool {
        self.gate.0.lock().expect("operation gate").active == Some(self.token)
    }
    pub fn permit(&self) -> MutationPermit {
        MutationPermit {
            gate: self.gate.clone(),
            token: self.token,
        }
    }
}
impl Drop for MutationLease {
    fn drop(&mut self) {
        let mut state = self.gate.0.lock().expect("operation gate");
        state.release(self.token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]
        #[test]
        fn generations_never_reuse_identity_or_release_new_owners(actions in prop::collection::vec((any::<bool>(), 1u64..32), 1..64)) {
            let mut state = GateState::new("process".to_owned());
            let mut sessions = BTreeSet::from([state.session.clone()]);
            let mut ids = BTreeSet::new();
            for (replace, id) in actions {
                if replace {
                    let old = state.acquire_token().unwrap();
                    state.rotate();
                    prop_assert!(sessions.insert(state.session.clone()));
                    prop_assert!(state.session.len() <= 128);
                    let current = state.acquire_token().unwrap();
                    state.release(old);
                    prop_assert_eq!(state.active, Some(current));
                    state.release(current);
                    ids.clear();
                }
                if ids.insert(id) { state.receipts.insert(id); }
                prop_assert_eq!(&state.receipts.ids, &ids);
            }
        }
    }

    #[test]
    fn receipt_eviction_retains_the_last_window() {
        let mut receipts = Receipts::default();
        for id in 1..=RECEIPT_CAPACITY as u64 + 2 {
            receipts.insert(id);
        }
        assert!(!receipts.ids.contains(&1));
        assert!(!receipts.ids.contains(&2));
        assert_eq!(receipts.ids.len(), RECEIPT_CAPACITY);
        assert_eq!(receipts.ids.first(), Some(&3));
    }

    #[test]
    fn concurrent_replacement_and_old_release_preserve_the_new_owner() {
        loom::model(|| {
            let mut initial = GateState::new("process".to_owned());
            let old = initial.acquire_token().unwrap();
            let state = loom::sync::Arc::new(loom::sync::Mutex::new(initial));
            let release = state.clone();
            let thread = loom::thread::spawn(move || {
                release.lock().unwrap().release(old);
            });
            let current = {
                let mut state = state.lock().unwrap();
                state.rotate();
                let current = state.acquire_token().unwrap();
                assert_ne!(current, old);
                current
            };
            thread.join().unwrap();
            assert_eq!(state.lock().unwrap().active, Some(current));
        });
    }

    #[test]
    fn concurrent_admission_and_suspension_leave_no_current_permit() {
        loom::model(|| {
            let state =
                loom::sync::Arc::new(loom::sync::Mutex::new(GateState::new("process".to_owned())));
            let admitted = state.clone();
            let thread = loom::thread::spawn(move || {
                let mut state = admitted.lock().unwrap();
                match state.acquire_token() {
                    Ok(token) => {
                        state.release(token);
                    },
                    Err(error) => assert_eq!(error, StorybookAutomationError::NoLiveHost),
                }
            });
            {
                let mut state = state.lock().unwrap();
                state.suspend();
            }
            thread.join().unwrap();
            let mut state = state.lock().unwrap();
            assert!(state.suspended);
            assert_eq!(state.active, None);
            assert_eq!(
                state.acquire_token().unwrap_err(),
                StorybookAutomationError::NoLiveHost
            );
        });
    }
}
