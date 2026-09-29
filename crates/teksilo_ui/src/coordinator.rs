//! Coalesced, thread-safe notifications for the UI coordinator. No timer thread.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use teksilo::core::AppEventPoster;

#[derive(Clone, Default)]
pub struct CoordinatorWake(Arc<State>);
#[derive(Default)]
struct State {
    pending: AtomicBool,
    poster: Mutex<Option<Arc<dyn AppEventPoster>>>,
}
pub struct CoordinatorEvent(CoordinatorWake);
impl CoordinatorWake {
    pub fn notify(&self) {
        if !self.0.pending.swap(true, Ordering::AcqRel) {
            self.post();
        }
    }
    fn post(&self) {
        let poster = self.0.poster.lock().unwrap().clone();
        if let Some(poster) = poster {
            poster.post_external(Box::new(CoordinatorEvent(self.clone())));
        }
    }
    pub fn attach(&self, poster: Option<Arc<dyn AppEventPoster>>) {
        *self.0.poster.lock().unwrap() = poster;
        if self.0.pending.load(Ordering::Acquire) {
            self.post();
        }
    }
    pub fn take(&self) -> bool {
        self.0.pending.swap(false, Ordering::AcqRel)
    }
    pub fn owns(&self, event: &CoordinatorEvent) -> bool {
        Arc::ptr_eq(&self.0, &event.0.0)
    }
}
