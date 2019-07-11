# threadpool-blocking

**_ This crate requires Rust nightly. _**

### Usage

```rust
use futures::future::{FutureExt, TryFutureExt};
use threadpool_blocking::{blocking, BlockingIterator};

fn main() {
    let test_fut = async move {
        assert_eq!(blocking(|| 2077 as usize).await, Ok(2077));

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
```
