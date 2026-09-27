//! Stable entity identities for navigation; visible row indices are never selection keys.
use crate::session_vm::Row;
use std::rc::Rc;
use teksilo::{
    core::signal::ObserverHandle,
    data::{
        DataChange, DragEligibility, DragSource, DropCommit, DropPosition, DropQuery, DropResponse,
        ListDataSource, ListModel,
    },
};

/// Moves the row `key` before or after the row `target`, in the store.
/// Returns whether it moved.
pub type Reorder = Rc<dyn Fn(u64, u64, DropPosition) -> bool>;

#[derive(Clone)]
pub struct NavigationSource(pub ListModel<Row>, pub Option<Reorder>);
impl ListDataSource for NavigationSource {
    type Item = Row;
    type Key = u64;
    fn len(&self) -> usize {
        self.0.len()
    }
    fn with_item<R>(&self, index: usize, f: impl FnOnce(&Row) -> R) -> Option<R> {
        self.0.with_item(index, f)
    }
    fn key_at(&self, index: usize) -> Option<u64> {
        self.0.with_item(index, |r| r.id)
    }
    fn index_of(&self, key: &u64) -> Option<usize> {
        (0..self.0.len()).find(|i| self.key_at(*i) == Some(*key))
    }
    fn observe_changes(&self, f: impl Fn(&DataChange) + 'static) -> ObserverHandle {
        self.0.observe_changes(f)
    }
    fn drag(&self, _key: &u64) -> DragEligibility {
        if self.1.is_some() {
            DragEligibility::CanDrag
        } else {
            DragEligibility::NoDrag
        }
    }
    /// Rows reorder among themselves; nothing drops *into* a row.
    fn can_accept(&self, query: &DropQuery<'_, u64>) -> DropResponse {
        match (&query.source, query.position, &self.1) {
            (DragSource::SameView { .. }, DropPosition::Before | DropPosition::After, Some(_)) => {
                DropResponse::Accept
            }
            _ => DropResponse::Reject,
        }
    }
    fn accept_drop(&self, commit: DropCommit<'_, u64>) -> bool {
        match (&commit.source, &self.1) {
            (DragSource::SameView { key }, Some(reorder))
                if commit.position != DropPosition::Into =>
            {
                reorder(*key, commit.target, commit.position)
            }
            _ => false,
        }
    }
}
