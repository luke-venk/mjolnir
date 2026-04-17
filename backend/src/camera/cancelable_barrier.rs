use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BarrierResult {
    Released,
    Canceled,
}

struct CancelableBarrierState {
    count: usize,
    total: usize,
    cancelled: bool,
}

#[derive(Clone)]
pub struct CancelableBarrier {
    inner: Arc<(Mutex<CancelableBarrierState>, Condvar)>,
}

impl CancelableBarrier {
    pub fn new(total: usize) -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(CancelableBarrierState {
                    count: 0,
                    total,
                    cancelled: false,
                }),
                Condvar::new(),
            )),
        }
    }

    pub fn wait(&self) -> BarrierResult {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().unwrap();

        // If already cancelled, return immediately
        if state.cancelled {
            return BarrierResult::Canceled;
        }

        state.count += 1;

        // If this thread completes the barrier, release everyone
        if state.count == state.total {
            state.count = 0; // optional reset behavior
            cvar.notify_all();
            return BarrierResult::Released;
        }

        // Otherwise wait until release or cancel
        loop {
            state = cvar.wait(state).unwrap();

            if state.cancelled {
                return BarrierResult::Canceled;
            }

            if state.count == 0 {
                return BarrierResult::Released;
            }
        }
    }

    pub fn cancel(&self) {
        let (lock, cvar) = &*self.inner;
        let mut state = lock.lock().unwrap();

        state.cancelled = true;
        cvar.notify_all();
    }
}
