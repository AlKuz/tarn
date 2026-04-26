use std::sync::Arc;

use tokio::sync::mpsc;
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
/// Creates an in-memory event queue (mpsc channel) for live observer events.
/// Review events are processed directly and synchronously before returning,
/// so the caller can rely on a ready index after return.
///
/// Startup order:
/// 1. Consumer task is spawned to drain the channel.
/// 2. Observer is spawned, sending live events into the channel.
/// 3. Review events are processed inline (not through the channel).
///
/// Duplicate events (observer re-reports a change that review already handled)
/// are safe because `update_index`/`delete_index` are idempotent.
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

    // Task 1: Consumer ← queue
    // Spawned first so it is ready to drain observer events.
    let consumer_core = core.clone();
    let consumer_task = tokio::spawn(async move {
        consume_events(consumer_core, rx).await;
    });

    // Task 2: Observer → queue
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
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "observer event error");
                }
            }
        }
    });

    // Process review events directly — index is ready when this loop finishes.
    tracing::info!("reviewing changes...");
    let review_stream = core.review_changes().await?;
    tokio::pin!(review_stream);
    while let Some(event) = review_stream.next().await {
        match event {
            Ok(event) => process_event(core, &event).await,
            Err(e) => tracing::warn!(error = %e, "review event error"),
        }
    }
    tracing::info!("changes reviewed");
    tracing::info!("index sync started");

    Ok(SyncHandle::new(vec![observer_task, consumer_task]))
}

async fn consume_events<S, I, O, R>(
    core: Arc<TarnCore<S, I, O, R>>,
    mut rx: mpsc::Receiver<StorageEvent>,
) where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    while let Some(event) = rx.recv().await {
        process_event(&core, &event).await;
    }
    tracing::debug!("event consumer finished (all senders dropped)");
}

async fn process_event<S, I, O, R>(core: &Arc<TarnCore<S, I, O, R>>, event: &StorageEvent)
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    match event {
        StorageEvent::Created { path, .. } | StorageEvent::Updated { path, .. } => {
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
        StorageEvent::Deleted { path } => {
            if let Err(e) = core.delete_index(path).await {
                tracing::warn!(path = %path, error = %e, "failed to delete from index");
            }
        }
    }
}
