use crate::log::{LogEntry, LogOpts};
use crate::spec::BranchSpec;
use crate::{CommitLogEntry, CommitLogIterator, TDBInstanceDeserializer, TerminusDBHttpClient};
use ::log::warn;
use futures_util::{Future, Stream, StreamExt, TryStreamExt};
use terminusdb_schema::ToTDBInstance;
use std::pin::Pin;
use std::task::{Context, Poll};

/// iterator that walks the commit log and returns all entities of a specific type found in it
#[derive(Debug)]
pub struct EntityIterator<T: ToTDBInstance, Deser: TDBInstanceDeserializer<T>> {
    /// the underlying commit log iterator that we go over to parse entities from
    log_iter: CommitLogIterator,

    /// deserializer for turning TDB instances into Parture models
    deserializer: Deser,

    /// buffer of entities that we read from the commit log
    buffer: Vec<T>,

    /// current commit log entry that we are currently returning entities for
    active_log_entry: Option<LogEntry>,

    _type: std::marker::PhantomData<T>,
}

impl<T: ToTDBInstance, Deser: TDBInstanceDeserializer<T>> EntityIterator<T, Deser> {
    pub fn new(log_iter: CommitLogIterator, deserializer: Deser) -> Self {
        Self {
            log_iter,
            deserializer,
            buffer: vec![],
            active_log_entry: None,
            _type: Default::default(),
        }
    }
}

impl<T: ToTDBInstance + 'static + Unpin, Deser: TDBInstanceDeserializer<T> + 'static + Unpin> Stream
    for EntityIterator<T, Deser>
where
    T: Send,
    Deser: Send,
{
    type Item = anyhow::Result<(T, LogEntry)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Safe to get_mut directly since we require T and Deser to be Unpin
        let this = self.as_mut().get_mut();

        // If we have buffered entities, return the next one
        if !this.buffer.is_empty() {
            let entity = this.buffer.remove(0);
            let log_entry = this.active_log_entry.clone().unwrap();
            return Poll::Ready(Some(Ok((entity, log_entry))));
        }

        // Get the next log entry from the iterator
        let mut log_iter = Pin::new(&mut this.log_iter);
        match log_iter.poll_next(cx) {
            Poll::Ready(Some(Ok(log_entry))) => {
                this.active_log_entry = Some(log_entry.clone());

                // Start fetching entities for this log entry
                let entities_future = this.log_iter.client().all_commit_created_entities::<T>(
                    this.log_iter.spec(),
                    &log_entry,
                    &mut this.deserializer,
                );

                // Poll the future directly
                let mut boxed_future = Box::pin(entities_future);
                match boxed_future.as_mut().poll(cx) {
                    Poll::Ready(Ok(entities)) => {
                        this.buffer = entities;

                        if this.buffer.is_empty() {
                            // No entities found in this log entry, try the next one by returning Pending
                            // The Stream will be polled again immediately
                            return Poll::Pending;
                        } else {
                            // We have entities, return the first one
                            let entity = this.buffer.remove(0);
                            let log_entry = this.active_log_entry.clone().unwrap();
                            Poll::Ready(Some(Ok((entity, log_entry))))
                        }
                    }
                    Poll::Ready(Err(err)) => {
                        warn!("error reading entities from commit: {:#?}", err);
                        Poll::Ready(Some(Err(err)))
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            Poll::Ready(Some(Err(err))) => {
                warn!("error reading log entry: {:#?}", err);
                Poll::Ready(Some(Err(err)))
            }
            Poll::Ready(None) => {
                // No more log entries
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

// Helper methods for using the Stream
impl<T: ToTDBInstance + 'static + Unpin, Deser: TDBInstanceDeserializer<T> + 'static + Unpin>
    EntityIterator<T, Deser>
where
    T: Send,
    Deser: Send,
{
    /// Collects all entities into a Vec
    pub async fn collect_all(&mut self) -> anyhow::Result<Vec<(T, LogEntry)>> {
        let mut results = Vec::new();
        let mut pinned = Pin::new(self);
        while let Some(result) = StreamExt::next(&mut pinned).await {
            results.push(result?);
        }
        Ok(results)
    }

    /// Returns the next item in the stream
    pub async fn next(&mut self) -> Option<anyhow::Result<(T, LogEntry)>> {
        StreamExt::next(&mut Pin::new(self)).await
    }
}
