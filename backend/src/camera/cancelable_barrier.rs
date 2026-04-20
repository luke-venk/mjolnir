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

/// This works just like a regular `std::sync::Barrier`
/// However, when any thread calls `cancel()` on the barrier, it stops blocking for all threads that have called `wait()` on the barrier.
/// This way, we can exit cleanly when canceling operations by canceling blocking barriers on all threads.
/// Without this struct, if we press Ctrl + C during a PTP sync, a thread may be out of sync and waiting indefinitely on barrier.
/// In that case, the program would not exit.
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
