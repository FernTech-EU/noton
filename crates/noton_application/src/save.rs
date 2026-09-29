//! A single writer owns the file lock. Acknowledgements always name their snapshot.
use anyhow::{Context, Result};
use noton_format::{Library, LibraryFile};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};
pub struct SaveReply {
    pub sequence: Option<u64>,
    pub result: Result<()>,
}
enum Request {
    Save(u64, Library),
    Backup(PathBuf),
    Stop,
}
type CompletionWake = Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>;
pub struct SaveWorker {
    wake: CompletionWake,
    sender: Sender<Request>,
    receiver: Receiver<SaveReply>,
    thread: Option<JoinHandle<()>>,
}
impl SaveWorker {
    pub fn start(mut file: LibraryFile) -> Self {
        let (tx, rx) = mpsc::channel();
        let (done, out) = mpsc::channel();
        let wake: CompletionWake = Arc::default();
        let completion = wake.clone();
        let thread = thread::spawn(move || {
            while let Ok(request) = rx.recv() {
                match request {
                    Request::Save(sequence, snapshot) => {
                        let result = file.save(&snapshot);
                        if done
                            .send(SaveReply {
                                sequence: Some(sequence),
                                result,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Request::Backup(path) => {
                        let result = file.backup(&path);
                        if done
                            .send(SaveReply {
                                sequence: None,
                                result,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Request::Stop => break,
                }
                let notify = completion.lock().unwrap().clone();
                if let Some(notify) = notify {
                    notify();
                }
            }
        });
        Self {
            wake,
            sender: tx,
            receiver: out,
            thread: Some(thread),
        }
    }
    pub fn set_completion_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        *self.wake.lock().unwrap() = Some(wake);
    }
    pub fn save(&self, sequence: u64, snapshot: Library) -> Result<()> {
        self.sender
            .send(Request::Save(sequence, snapshot))
            .context("Save worker stopped")
    }
    pub fn backup(&self, path: PathBuf) -> Result<()> {
        self.sender
            .send(Request::Backup(path))
            .context("Save worker stopped")
    }
    pub fn poll(&self) -> Option<SaveReply> {
        self.receiver.try_recv().ok()
    }
    pub fn wait(&self) -> Result<SaveReply> {
        self.receiver.recv().context("Save worker stopped")
    }
}
impl Drop for SaveWorker {
    fn drop(&mut self) {
        let _ = self.sender.send(Request::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
