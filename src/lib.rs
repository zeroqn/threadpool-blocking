#![feature(async_await)]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::channel::mpsc;
use futures::future::{join_all, FutureExt, TryFutureExt};
use futures::stream::StreamExt;

pub struct Blocking<F: FnMut() -> R + Unpin, R> {
    f: F,
}

impl<F: FnMut() -> R + Unpin, R> Blocking<F, R> {
    pub fn new(f: F) -> Self {
        Blocking { f }
    }
}

impl<F: FnMut() -> R + Unpin, R> Future for Blocking<F, R> {
    type Output = R;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
        use futures01::Async;
        use tokio_threadpool::blocking;

        match blocking(&mut (self.get_mut().f)) {
            Ok(Async::Ready(ret)) => Poll::Ready(ret),
            Ok(Async::NotReady) => Poll::Pending,
            Err(_e) => panic!("Use blocking outside threadpool"),
        }
    }
}

pub trait ParBlocking {
    fn par_blocking<F, R>(self, f: F) -> Box<dyn Future<Output = Vec<Option<R>>> + Unpin>
    where
        Self: Iterator,
        F: FnMut(<Self as Iterator>::Item) -> R + Send + Unpin + 'static + Clone,
        <Self as Iterator>::Item: Unpin + Send + 'static + Clone,
        R: Send + 'static;
}

impl<I: Iterator> ParBlocking for I {
    fn par_blocking<F, R>(self, f: F) -> Box<dyn Future<Output = Vec<Option<R>>> + Unpin>
    where
        F: FnMut(<Self as Iterator>::Item) -> R + Send + Unpin + 'static + Clone,
        <Self as Iterator>::Item: Unpin + Send + 'static + Clone,
        R: Send + 'static,
    {
        let mut futs = Vec::new();

        for item in self {
            let (mut done, mut done_rx) = mpsc::channel::<R>(0);
            let mut f = f.clone();

            let blocking = Blocking::new(move || {
                let _ = done.try_send(f(item.clone()));
            });

            tokio_executor::spawn(blocking.unit_error().boxed().compat());

            futs.push(async move { done_rx.next().await })
        }

        Box::new(join_all(futs))
    }
}

#[cfg(test)]
mod tests {
    use super::{Blocking, ParBlocking};

    use futures::channel::mpsc;
    use futures::executor::block_on;
    use futures::future::{FutureExt, TryFutureExt};
    use futures::stream::StreamExt;
    use tokio_threadpool::ThreadPool;

    #[test]
    fn should_be_spawned_in_tokio_threadpool() {
        let pool = ThreadPool::new();
        let (mut tx, mut rx) = mpsc::channel(0);

        let blocking = Blocking::new(move || {
            let _ = tx.try_send(());
        });
        pool.spawn(blocking.unit_error().boxed().compat());

        assert_eq!(block_on(rx.next()), Some(()));
    }

    #[test]
    fn should_do_batch_blocking_on_iterator() {
        let pool = ThreadPool::new();

        let mut enter = tokio_executor::enter().unwrap();
        tokio_executor::with_default(&mut pool.sender(), &mut enter, |_enter| {
            let fut = (0..1000).into_iter().par_blocking(|_| 1);

            assert_eq!(block_on(fut).iter().any(|r| *r != Some(1)), false);
        });
    }
}
