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
    use super::{blocking, BlockingIterator};

    use futures::executor::block_on;
    use futures::future::{FutureExt, TryFutureExt};
    use tokio_threadpool::ThreadPool;

    #[test]
    fn should_be_spawned_in_tokio_threadpool() {
        let pool = ThreadPool::new();

        let mut enter = tokio_executor::enter().unwrap();
        tokio_executor::with_default(&mut pool.sender(), &mut enter, |_enter| {
            assert_eq!(block_on(blocking(|| 2077)), Ok(2077));
        });
    }

    #[test]
    fn should_work_within_tokio() {
        let test_fut = async move {
            // test blocking func
            assert_eq!(blocking(|| 2077 as usize).await, Ok(2077));

            // test pool_chunk_blocking on iterator
            let blocking_collection = (0..99).pool_chunk_blocking(10, |chunk| chunk.len()).await;
            let len = (99 / 10) + 1;

            assert_eq!(
                blocking_collection
                    .iter()
                    .take(len - 1)
                    .any(|r| *r != Ok(10)),
                false
            );
            assert_eq!(blocking_collection.last(), Some(&Ok(9)));
        };

        tokio::run(test_fut.unit_error().boxed().compat());
    }
}
