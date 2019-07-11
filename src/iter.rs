use crate::Blocking;

use futures::channel::oneshot::{self, Canceled};
use futures::future::{join_all, FutureExt, TryFutureExt};

use std::future::Future;

pub type BlockingCollectionFuture<T> =
    Box<dyn Future<Output = Vec<Result<T, Canceled>>> + Send + Unpin>;

pub trait BlockingIterator {
    type Item;

    fn pool_blocking<F, R>(self, f: F) -> BlockingCollectionFuture<R>
    where
        F: FnOnce(Self::Item) -> R + Clone + Send + 'static,
        R: Send + 'static;

    fn pool_chunk_blocking<F, R>(self, chunk_size: usize, f: F) -> BlockingCollectionFuture<R>
    where
        F: FnOnce(Vec<Self::Item>) -> R + Clone + Send + 'static,
        R: Send + 'static;
}

impl<I> BlockingIterator for I
where
    I: Iterator,
    <I as Iterator>::Item: Send + Clone + 'static,
{
    type Item = <I as Iterator>::Item;

    fn pool_blocking<F, R>(self, f: F) -> BlockingCollectionFuture<R>
    where
        F: FnOnce(Self::Item) -> R + Clone + Send + 'static,
        R: Send + 'static,
    {
        self.pool_chunk_blocking(1, |mut chunk| {
            let item = chunk.pop().expect("fail to pop item from chunk");

            f(item)
        })
    }

    fn pool_chunk_blocking<F, R>(self, chunk_size: usize, f: F) -> BlockingCollectionFuture<R>
    where
        F: FnOnce(Vec<Self::Item>) -> R + Clone + Send + 'static,
        R: Send + 'static,
    {
        let mut chunk = Vec::with_capacity(chunk_size);
        let mut result_fut = vec![];

        let mut pool_chunk = |f: F, chunk| {
            let (done, done_rx) = oneshot::channel();

            let blocking = Blocking::new(done, move || f(chunk));
            tokio_executor::spawn(blocking.unit_error().boxed().compat());

            result_fut.push(done_rx);
        };

        for item in self {
            if chunk.len() == chunk.capacity() {
                let chunk = std::mem::replace(&mut chunk, Vec::with_capacity(chunk_size));

                pool_chunk(f.clone(), chunk);
            }

            chunk.push(item);
        }

        if !chunk.is_empty() {
            pool_chunk(f, chunk);
        }

        Box::new(join_all(result_fut))
    }
}

#[cfg(test)]
mod tests {
    use super::BlockingIterator;

    use futures::executor::block_on;
    use tokio_threadpool::ThreadPool;

    #[test]
    fn should_blocking_on_iterator() {
        let pool = ThreadPool::new();

        let mut enter = tokio_executor::enter().unwrap();
        tokio_executor::with_default(&mut pool.sender(), &mut enter, |_enter| {
            let blocking_fut = (0..10).pool_blocking(|_| 1);

            let result = block_on(blocking_fut);
            assert_eq!(result.len(), 10);
            assert_eq!(result.iter().any(|r| *r != Ok(1)), false);
        });
    }

    #[test]
    fn should_do_chunk_blocking_on_iterator() {
        let pool = ThreadPool::new();

        let mut enter = tokio_executor::enter().unwrap();
        tokio_executor::with_default(&mut pool.sender(), &mut enter, |_enter| {
            let blocking_fut = (0..99).pool_chunk_blocking(10, |chunk| chunk.len());

            let result = block_on(blocking_fut);
            let len = (99 / 10) + 1;

            assert_eq!(result.len(), len);
            assert_eq!(result.iter().take(len - 1).any(|r| *r != Ok(10)), false);
            assert_eq!(result.last(), Some(&Ok(9)));
        });
    }
}
