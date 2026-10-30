//! Bounded coordination for disposable example workers, never production authority.
use std::sync::{Condvar, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitError {
    Aborted,
    TimedOut,
    Poisoned,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Snapshot {
    pub arrivals: usize,
    pub aborted: bool,
    pub timed_out: bool,
}

pub struct Rendezvous {
    state: Mutex<Snapshot>,
    changed: Condvar,
    timeout: Duration,
}
impl Rendezvous {
    pub fn new(timeout: Duration) -> Self {
        Self {
            state: Mutex::new(Snapshot::default()),
            changed: Condvar::new(),
            timeout,
        }
    }
    pub fn arrive(&self) -> Result<(), WaitError> {
        let mut state = self.state.lock().map_err(|_| WaitError::Poisoned)?;
        if state.aborted {
            return Err(WaitError::Aborted);
        }
        if state.timed_out {
            return Err(WaitError::TimedOut);
        }
        state.arrivals += 1;
        if state.arrivals == 2 {
            self.changed.notify_all();
            return Ok(());
        }
        let (mut state, _) = self
            .changed
            .wait_timeout_while(state, self.timeout, |s| {
                s.arrivals < 2 && !s.aborted && !s.timed_out
            })
            .map_err(|_| WaitError::Poisoned)?;
        if state.aborted {
            return Err(WaitError::Aborted);
        }
        if state.arrivals == 2 {
            return Ok(());
        }
        state.timed_out = true;
        self.changed.notify_all();
        Err(WaitError::TimedOut)
    }
    /// An early returned error must release a peer; normal completion after both
    /// claims have arrived must not revoke their already established rendezvous.
    pub fn participant_finished(&self) {
        if let Ok(mut state) = self.state.lock()
            && state.arrivals < 2
            && !state.timed_out
        {
            state.aborted = true;
            self.changed.notify_all();
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot, WaitError> {
        self.state
            .lock()
            .map(|s| s.clone())
            .map_err(|_| WaitError::Poisoned)
    }
}
