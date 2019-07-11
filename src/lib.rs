#![feature(async_await)]

mod blocking;
pub mod iter;
pub use iter::BlockingIterator;

use blocking::Blocking;

use futures::channel::oneshot::{self, Canceled};
use futures::future::{FutureExt, TryFutureExt};

use std::future::Future;

pub fn blocking<F, R>(f: F) -> impl Future<Output = Result<R, Canceled>>
where
    F: FnOnce() -> R + Clone + Send + 'static,
    R: Send + 'static,
{
    let (done, done_rx) = oneshot::channel();
    let blocking = Blocking::new(done, f);

    tokio_executor::spawn(blocking.unit_error().boxed().compat());
    done_rx
}

#[cfg(test)]
mod tests {
    use super::blocking;

    use futures::executor::block_on;
    use tokio_threadpool::ThreadPool;

    #[test]
    fn should_be_spawned_in_tokio_threadpool() {
        let pool = ThreadPool::new();

        let mut enter = tokio_executor::enter().unwrap();
        tokio_executor::with_default(&mut pool.sender(), &mut enter, |_enter| {
            assert_eq!(block_on(blocking(|| 2077)), Ok(2077));
        });
    }
}
