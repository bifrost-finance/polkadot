// Copyright 2017-2021 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Tracked variant of channels for better metrics.

use std::sync::atomic::{AtomicUsize, Ordering};

use futures::channel::mpsc;
use futures::task::Poll;
use futures::task::Context;
use futures::sink::SinkExt;
use futures::stream::{Stream, StreamExt};

use std::result;
use std::sync::Arc;
use std::pin::Pin;


/// Create a wrapped `mpsc::channel` pair of `TrackedSender` and `TrackedReceiver`.
pub fn channel<T>(capacity: usize, name: &'static str) -> (TrackedSender<T>, TrackedReceiver<T>) {
    let (tx, rx) = mpsc::channel(capacity);
    let shared_cntr = Arc::new(AtomicUsize::default());

    let tx = TrackedSender {
        fill: Arc::clone(&shared_cntr),
        inner: tx,
        name,
    };
    let rx = TrackedReceiver {
        fill: shared_cntr,
        inner: rx,
        name,
    };
    (tx, rx)
}


#[derive(Debug)]
pub struct TrackedReceiver<T> {
    // count currently contained messages
    fill: Arc<AtomicUsize>,
    inner: mpsc::Receiver<T>,
    name: &'static str,
}

impl<T> std::ops::Deref for TrackedReceiver<T> {
    type Target = mpsc::Receiver<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for TrackedReceiver<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> Stream for TrackedReceiver<T> {
    type Item = T;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match mpsc::Receiver::poll_next(Pin::new(&mut self.inner), cx) {
            Poll::Ready(x) => {
                // FIXME run over should be cought I guess
                self.fill.fetch_sub(1, Ordering::SeqCst);
                Poll::Ready(x)
            },
            other => other,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T> TrackedReceiver<T> {
    fn try_next(&mut self) -> Result<Option<T>, mpsc::TryRecvError> {
        match self.inner.try_next()? {
            Some(x) => {
                self.fill.fetch_sub(1, Ordering::SeqCst);
                Ok(Some(x))
            },
            None => Ok(None),
        }
    }
}

#[derive(Debug,Clone)]
pub struct TrackedSender<T> {
    fill: Arc<AtomicUsize>,
    inner: mpsc::Sender<T>,
    name: &'static str,
}

impl<T> std::ops::Deref for TrackedSender<T> {
    type Target = mpsc::Sender<T>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for TrackedSender<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> TrackedSender<T> {
    pub async fn send(&mut self, item: T) -> result::Result<(), mpsc::SendError> where Self: Unpin {
        self.fill.fetch_add(1, Ordering::SeqCst);
        let fut = self.inner.send(item);
        futures::pin_mut!(fut);
        fut.await
    }

    pub fn try_send(&mut self, msg: T) -> result::Result<(), mpsc::TrySendError<T>> {
        self.inner.try_send(msg)?;
        self.fill.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_counter_many_tasks() {

    }
}
