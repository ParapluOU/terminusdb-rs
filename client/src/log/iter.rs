use crate::log::{LogEntry, LogOpts};
use crate::spec::BranchSpec;
use crate::{CommitLogEntry, TerminusDBHttpClient};
use derive_getters::Getters;
use futures_util::{Future, Stream, StreamExt, TryStreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Getters, Debug, Clone)]
pub struct CommitLogIterator {
    /// TerminusDB HTTP client
    client: TerminusDBHttpClient,
    /// the target db and branch to read from
    spec: BranchSpec,
    /// options for quering the logs like batch size
    opts: LogOpts,
    /// current buffer of entries that we are returning
    buffer: Vec<LogEntry>,
    /// last offset of the branch that we read from
    last_offset_read: Option<usize>,
}

impl CommitLogIterator {
    pub fn new(client: TerminusDBHttpClient, spec: BranchSpec, opts: LogOpts) -> Self {
        CommitLogIterator {
            client,
            spec,
            opts,
            buffer: Vec::new(),
            last_offset_read: None,
        }
    }

    pub fn batch_size(&self) -> usize {
        self.opts.count.unwrap_or(10)
    }

    /// Collects all log entries into a Vec
    pub async fn collect_all(&mut self) -> anyhow::Result<Vec<LogEntry>> {
        let mut results = Vec::new();
        let mut pinned = Pin::new(self);
        while let Some(result) = StreamExt::next(&mut pinned).await {
            results.push(result?);
        }
        Ok(results)
    }

    /// Returns the next item in the stream
    pub async fn next(&mut self) -> Option<anyhow::Result<LogEntry>> {
        StreamExt::next(&mut Pin::new(self)).await
    }
}

impl Stream for CommitLogIterator {
    type Item = anyhow::Result<LogEntry>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // Use already buffered entries if available
        if !this.buffer.is_empty() {
            return Poll::Ready(Some(Ok(this.buffer.pop().unwrap())));
        }

        // not first read but no results left
        if this.last_offset_read.is_some() && this.buffer.is_empty() {
            return Poll::Ready(None);
        }

        // Remember offset that we read from
        this.last_offset_read = this.opts.offset;

        // Create future for fetching logs
        let logs_future = this.client.log(&this.spec, this.opts.clone());

        // Poll the future directly
        let mut boxed_future = Box::pin(logs_future);
        match boxed_future.as_mut().poll(cx) {
            Poll::Ready(Ok(entries)) => {
                if entries.is_empty() {
                    return Poll::Ready(None);
                }

                // Update offset for next batch
                this.opts.offset = this
                    .opts
                    .offset
                    .map(|v| v + this.batch_size())
                    .unwrap_or(this.batch_size())
                    .into();

                // Update buffer
                this.buffer = entries;

                // Return first item
                Poll::Ready(Some(Ok(this.buffer.pop().unwrap())))
            }
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}
