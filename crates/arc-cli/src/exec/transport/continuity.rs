use std::cell::{Cell, RefCell};

/// A daemon instance owns a single database connection for its lifetime.
/// A changed or unidentifiable instance cannot confirm the original store.
#[derive(Default)]
pub(in crate::exec) struct Continuity {
    first: RefCell<Option<arc_core::process_identity::ProcessIdentity>>,
    ambiguous: Cell<bool>,
}
impl Continuity {
    pub(in crate::exec) fn record(
        &self,
        identity: Option<arc_core::process_identity::ProcessIdentity>,
    ) {
        let Some(identity) = identity else {
            self.ambiguous.set(true);
            return;
        };
        let mut first = self.first.borrow_mut();
        if let Some(first) = first.as_ref() {
            if *first != identity {
                self.ambiguous.set(true);
            }
        } else {
            *first = Some(identity);
        }
    }
    pub(in crate::exec) fn confirmed(&self) -> bool {
        !self.ambiguous.get() && self.first.borrow().is_some()
    }
}
