use futures::channel::oneshot::Sender;

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(super) struct Blocking<F, R>
where
    F: FnOnce() -> R + Clone,
    R: Send,
{
    done_tx: Cell<Option<Sender<R>>>,
    func: F,
}

impl<F, R> Blocking<F, R>
where
    F: FnOnce() -> R + Clone,
    R: Send,
{
    pub fn new(done_tx: Sender<R>, func: F) -> Self {
        Blocking {
            done_tx: Cell::new(Some(done_tx)),
            func,
        }
    }
}

impl<F, R> Future for Blocking<F, R>
where
    F: FnOnce() -> R + Clone,
    R: Send + 'static,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        use futures01::Async;
        use tokio_threadpool::blocking;

        match blocking(self.func.clone()) {
            Ok(Async::Ready(ret)) => {
                if let Some(tx) = self.done_tx.take() {
                    if !tx.is_canceled() {
                        let _ = tx.send(ret);
                    }
                } else {
                    panic!("Poll completed blocking");
                }

                Poll::Ready(())
            }
            Ok(Async::NotReady) => Poll::Pending,
            Err(_e) => panic!("Use blocking outside threadpool"),
        }
    }
}
