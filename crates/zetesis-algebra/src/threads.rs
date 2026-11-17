//! The threads the solver starts, counted process-wide.
//!
//! `--threads N` bounds every thread the solver runs at once, the calling
//! thread included. Each spawn site starts its threads through this module:
//! [`spawn`] for a single named thread and [`Pool`] for a Rayon pool. A
//! [`Pool`] joins its threads when it drops, so a later stage's threads never
//! overlap an earlier stage's. Nothing in the solver uses Rayon's global pool;
//! the counters let a test check both properties.

use std::io;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{Builder, JoinHandle};

use rayon::{ThreadPool, ThreadPoolBuildError, ThreadPoolBuilder};

/// Live counted threads, the calling thread included.
static LIVE: AtomicUsize = AtomicUsize::new(1);
/// The most counted threads ever live at once.
static PEAK: AtomicUsize = AtomicUsize::new(1);

/// The counted threads live now, the calling thread included.
#[must_use]
pub fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

/// The most counted threads live at once since the process started, the
/// calling thread included.
#[must_use]
pub fn peak() -> usize {
    PEAK.load(Ordering::Relaxed)
}

/// Held by a counted thread for its whole body.
struct Counted;

impl Counted {
    fn enter() -> Self {
        let live = LIVE.fetch_add(1, Ordering::Relaxed) + 1;
        PEAK.fetch_max(live, Ordering::Relaxed);
        Self
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Start one counted, named thread.
///
/// # Errors
/// Returns the operating system's refusal to start the thread.
pub fn spawn<T: Send + 'static>(
    name: impl Into<String>,
    body: impl FnOnce() -> T + Send + 'static,
) -> io::Result<JoinHandle<T>> {
    #[allow(clippy::disallowed_methods, reason = "the one counted spawn")]
    Builder::new().name(name.into()).spawn(move || {
        let _counted = Counted::enter();
        body()
    })
}

/// A Rayon pool of counted threads that joins them when it drops.
///
/// Rayon's own pool only signals its threads to stop; joining them here makes
/// the pool's threads gone, not just going, once its owner is dropped.
pub struct Pool {
    pool: Option<ThreadPool>,
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl Pool {
    /// Start `threads` counted threads named by `name`.
    ///
    /// # Errors
    /// Returns the pool builder's error, including a refused thread.
    pub fn new(
        threads: NonZeroUsize,
        name: impl Fn(usize) -> String + 'static,
    ) -> Result<Self, ThreadPoolBuildError> {
        let handles = Arc::new(Mutex::new(Vec::with_capacity(threads.get())));
        let started = Arc::clone(&handles);
        #[allow(clippy::disallowed_methods, reason = "the counted, joined pool")]
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .thread_name(name)
            .spawn_handler(move |thread| {
                let mut builder = Builder::new();
                if let Some(name) = thread.name() {
                    builder = builder.name(name.to_owned());
                }
                if let Some(size) = thread.stack_size() {
                    builder = builder.stack_size(size);
                }
                #[allow(clippy::disallowed_methods, reason = "the counted pool's own spawn")]
                let handle = builder.spawn(move || {
                    let _counted = Counted::enter();
                    thread.run();
                })?;
                started
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(handle);
                Ok(())
            })
            .build();
        match pool {
            Ok(pool) => Ok(Self {
                pool: Some(pool),
                threads: handles,
            }),
            Err(error) => {
                // A partly built pool has already been dropped; reap what it started.
                join(&handles);
                Err(error)
            }
        }
    }
}

fn join(threads: &Mutex<Vec<JoinHandle<()>>>) {
    let threads = std::mem::take(&mut *threads.lock().unwrap_or_else(PoisonError::into_inner));
    for thread in threads {
        let _ = thread.join();
    }
}

impl Deref for Pool {
    type Target = ThreadPool;

    fn deref(&self) -> &ThreadPool {
        self.pool
            .as_ref()
            .expect("a pool is present until it drops")
    }
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Pool")
            .field("threads", &self.current_num_threads())
            .finish_non_exhaustive()
    }
}

impl Drop for Pool {
    /// Must not run on one of the pool's own threads, which would wait for itself.
    fn drop(&mut self) {
        drop(self.pool.take());
        join(&self.threads);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_pool_has_joined_its_counted_threads() {
        let pool = Pool::new(NonZeroUsize::new(3).unwrap(), |index| {
            format!("test-{index}")
        })
        .unwrap();
        pool.broadcast(|_| ());
        assert!(peak() >= 4);
        let threads = Arc::clone(&pool.threads);
        assert_eq!(threads.lock().unwrap().len(), 3);
        drop(pool);
        // Every handle was joined: the threads have finished, not just been told to.
        assert!(threads.lock().unwrap().is_empty());
    }

    #[test]
    fn a_spawned_thread_is_counted_while_it_runs() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (go, wait) = std::sync::mpsc::channel::<()>();
        let handle = spawn("test-counted", move || {
            tx.send(live()).unwrap();
            wait.recv().unwrap();
        })
        .unwrap();
        assert!(rx.recv().unwrap() >= 2);
        go.send(()).unwrap();
        handle.join().unwrap();
    }
}
