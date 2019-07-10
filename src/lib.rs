use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

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

#[cfg(test)]
mod tests {
    use super::Blocking;

    use futures::future::{FutureExt, TryFutureExt};
    use tokio_threadpool::ThreadPool;

    use std::sync::mpsc;

    #[test]
    fn should_be_spawned_in_tokio_threadpool() {
        let pool = ThreadPool::new();
        let (tx, rx) = mpsc::sync_channel(0);

        let blocking = Blocking::new(move || {
            let _ = tx.try_send(());
        });
        pool.spawn(blocking.unit_error().boxed().compat());

        assert_eq!(rx.recv(), Ok(()));
    }
}
