use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;

use crate::core::tarn_core::{CoreError, TarnCore};
use crate::index::Index;
use crate::observer::{Observer, StorageEvent};
use crate::revisions::RevisionTracker;
use crate::storage::Storage;

/// Handle to the background sync tasks.
///
/// Manages the observer and consumer tasks that keep the index
/// and revision tracker in sync with storage changes.
/// Dropping the handle aborts all background tasks.
pub struct SyncHandle {
    tasks: Vec<JoinHandle<()>>,
}

impl SyncHandle {
    fn new(tasks: Vec<JoinHandle<()>>) -> Self {
        Self { tasks }
    }

    /// Shut down all sync tasks.
    pub fn shutdown(self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Drop for SyncHandle {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Start the event sync pipeline.
///
/// Creates an in-memory event queue (mpsc channel) as a single event bus.
/// The observer and `review_changes()` both send events to this queue.
/// A single consumer task reads events and updates the index and revision tracker.
///
/// The observer is started **before** `review_changes()` to avoid missing
/// changes that occur during the review window. Duplicate events are handled
/// by the idempotency of `update_index`/`delete_index`.
///
/// This function blocks until all review events are fully processed (index
/// and revisions updated), so the caller can rely on a ready index after return.
pub async fn start_sync<S, I, O, R>(
    core: &Arc<TarnCore<S, I, O, R>>,
) -> Result<SyncHandle, CoreError>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    let (tx, rx) = mpsc::channel::<StorageEvent>(512);

    // Task 1: Observer → queue
    let observer_tx = tx.clone();
    let observer_core = core.clone();
    let observer_task = tokio::spawn(async move {
        let stream = match observer_core.listen_changes().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "failed to start file watcher");
                return;
            }
        };
        tokio::pin!(stream);
        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    if observer_tx.send(event).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "observer event error");
                }
            }
        }
    });

    // Reconcile offline changes → queue
    tracing::info!("reviewing changes...");
    let mut review_count: u64 = 0;
    {
        let review_stream = core.review_changes().await?;
        tokio::pin!(review_stream);
        while let Some(event) = review_stream.next().await {
            match event {
                Ok(event) => {
                    if tx.send(event).await.is_err() {
                        tracing::warn!("event consumer dropped during review");
                        break;
                    }
                    review_count += 1;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "review event error");
                }
            }
        }
    }
    drop(tx); // Only observer_tx remains as a sender

    // Task 2: Consumer ← queue
    // Uses a oneshot to signal when all review events have been processed.
    let (review_done_tx, review_done_rx) = oneshot::channel::<()>();
    let consumer_core = core.clone();
    let consumer_task = tokio::spawn(async move {
        consume_events(consumer_core, rx, review_count, review_done_tx).await;
    });

    // Block until all review events are indexed
    let _ = review_done_rx.await;
    tracing::info!("changes reviewed");
    tracing::info!("index sync started");

    Ok(SyncHandle::new(vec![observer_task, consumer_task]))
}

async fn consume_events<S, I, O, R>(
    core: Arc<TarnCore<S, I, O, R>>,
    mut rx: mpsc::Receiver<StorageEvent>,
    review_count: u64,
    review_done: oneshot::Sender<()>,
) where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    let mut processed: u64 = 0;
    let mut review_done = Some(review_done);

    // Signal immediately if there were no review events
    if review_count == 0
        && let Some(tx) = review_done.take()
    {
        let _ = tx.send(());
    }

    while let Some(event) = rx.recv().await {
        match event {
            StorageEvent::Created { ref path, .. } | StorageEvent::Updated { ref path, .. } => {
                match core.read(path).await {
                    Ok(file) => {
                        if let Err(e) = core.update_index(&file).await {
                            tracing::warn!(path = %path, error = %e, "failed to update index");
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            path = %path, error = %e,
                            "failed to read file for indexing (may have been deleted)"
                        );
                    }
                }
            }
            StorageEvent::Deleted { ref path } => {
                if let Err(e) = core.delete_index(path).await {
                    tracing::warn!(path = %path, error = %e, "failed to delete from index");
                }
            }
        }

        processed += 1;
        if review_done.is_some()
            && processed >= review_count
            && let Some(tx) = review_done.take()
        {
            let _ = tx.send(());
        }
    }

    // Safety: signal if consumer exits before reaching the count
    if let Some(tx) = review_done {
        let _ = tx.send(());
    }

    tracing::debug!("event consumer finished (all senders dropped)");
}
