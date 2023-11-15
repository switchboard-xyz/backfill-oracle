use crate::*;

use dashmap::DashMap;
use std::future::Future;
use std::sync::Arc;
use std::pin::Pin;

/// Function used to resolve a cached value based on a timestamp.
pub type FetchFunction<T> = Arc<
    dyn (Fn(i64) -> Pin<Box<dyn Future<Output = Result<T, SbError>> + Send + Sync>>) + Send + Sync
>;

/// A mapping of timestamps to a OnceCell that can only be initialized once.
type FutureMap<K, V> = DashMap<K, Arc<V>>;

type FutureOrValue<T> = RwLock<Option<Result<T, SbError>>>;

/// A cache of values based on a timestamp. Values in the map can only be resolved once.
#[derive(Clone)]
pub struct TimestampCache<T> where T: Copy + Sized + Send + Sync + 'static {
    data: FutureMap<i64, FutureOrValue<T>>,
    fetch_function: FetchFunction<T>,
}

impl<T> TimestampCache<T> where T: Copy + Sized + Send + Sync + 'static {
    pub fn new(fetch_function: FetchFunction<T>) -> Self {
        Self {
            data: DashMap::new(),
            fetch_function,
        }
    }

    pub fn set(&self, timestamp: i64, value: T) -> Result<(), SbError> {
        debug!("[CACHE] fetching value for timestamp {}", timestamp);

        self.data.insert(timestamp, Arc::new(RwLock::new(Option::Some(Result::Ok(value)))));

        Ok(())
    }

    /// Get or init a value based on the timestamp.
    pub async fn get(&self, timestamp: i64) -> Result<T, SbError> {
        let cell = self.data.entry(timestamp).or_default().value().clone();

        if let Some(v) = &*cell.read().await {
            return v.clone();
        }

        let mut lock = cell.write().await;

        match &*lock {
            Some(value) => value.clone(),
            None => {
                info!("[CACHE] fetching value for timestamp {}", timestamp);
                let fetcher = self.fetch_function.clone();
                match fetcher(timestamp).await {
                    Ok(value) => {
                        *lock = Some(Result::Ok(value));
                        drop(lock); // Drop the lock as soon as the value is set

                        // Return the fetched value
                        Ok(value)
                    }
                    Err(e) => {
                        error!("[CACHE] fetcher failed: {:?}", e);
                        Err(SbError::NetworkError)
                    }
                }
            }
        }
    }
}
