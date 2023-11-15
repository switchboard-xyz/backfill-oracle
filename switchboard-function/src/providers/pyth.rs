use crate::*;

use serde::{ Serialize, Deserialize };

pub const PYTH_BTC_FEED: &str = "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";

pub const PYTH_ETH_FEED: &str = "ff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace";

pub const PYTH_SOL_FEED: &str = "ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";

#[derive(Serialize, Deserialize, Debug)]
pub struct PythApiResponse {
    pub id: String,
    pub price: PythPriceInfo,
    pub ema_price: PythPriceInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PythPriceInfo {
    pub price: String,
    pub conf: String,
    pub expo: i32,
    pub publish_time: u64,
}
impl PythPriceInfo {
    pub fn to_fixed_precision(&self) -> u64 {
        let expo_diff = 9 - self.expo.abs(); // calculate the difference in precision
        let price = u64::from_str(&self.price).unwrap();
        if self.expo < 0 {
            price * (10u64).pow(expo_diff as u32)
        } else {
            price / (10u64).pow(expo_diff as u32)
        }
    }
}

async fn fetch_pyth_price(id: &str, timestamp: i64) -> Result<u64, SbError> {
    let env = WorkerEnvironment::get_or_init();
    let url = format!(
        "{}/api/get_price_feed?id={}&publish_time={}",
        env.pyth_rpc_url,
        id,
        timestamp
    );

    let response = reqwest::get(url).await.unwrap();

    if response.status().is_success() {
        let api_response: PythApiResponse = response.json().await.unwrap();
        let result = api_response.price.to_fixed_precision();
        Ok(result)
    } else {
        error!("[PYTH] Failed to fetch data: {}", response.status());
        Err(SbError::NetworkError)
    }
}

async fn fetch_pyth_prices(ids: Vec<&str>) -> Result<Vec<PythApiResponse>, SbError> {
    let env = WorkerEnvironment::get_or_init();
    let url = format!(
        "{}/api/latest_price_feeds?{}",
        env.pyth_rpc_url,
        ids
            .iter()
            .map(|s| format!("ids%5B%5D={}", s))
            .collect::<Vec<String>>()
            .join("&")
    );

    let response = reqwest::get(url).await.unwrap();

    if response.status().is_success() {
        let api_response: Vec<PythApiResponse> = response.json().await.unwrap();
        Ok(api_response)
    } else {
        error!("[PYTH] Failed to fetch data: {}", response.status());
        Err(SbError::NetworkError)
    }
}

#[derive(Clone)]
pub struct PythProvider {
    pub btc: TimestampCache<u64>,
    pub eth: TimestampCache<u64>,
    pub sol: TimestampCache<u64>,
}
impl std::default::Default for PythProvider {
    fn default() -> Self {
        Self {
            btc: TimestampCache::new(
                Arc::new(|timestamp| { Box::pin(fetch_pyth_price(PYTH_BTC_FEED, timestamp)) })
            ),
            eth: TimestampCache::new(
                Arc::new(|timestamp| { Box::pin(fetch_pyth_price(PYTH_ETH_FEED, timestamp)) })
            ),
            sol: TimestampCache::new(
                Arc::new(|timestamp| { Box::pin(fetch_pyth_price(PYTH_SOL_FEED, timestamp)) })
            ),
        }
    }
}
impl PythProvider {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn fetch(&self) -> Result<(), SbError> {
        let prices = fetch_pyth_prices(vec![PYTH_BTC_FEED, PYTH_ETH_FEED, PYTH_SOL_FEED]).await?;

        for price in prices {
            let timestamp: i64 = price.price.publish_time.try_into().unwrap();
            let value: u64 = price.price.to_fixed_precision();

            match price.id.as_str() {
                PYTH_BTC_FEED => {
                    self.btc.set(timestamp, value)?;
                    debug!("[PYTH] BTC-USD: {} => {:?}", timestamp, value);
                }
                PYTH_ETH_FEED => {
                    self.eth.set(timestamp, value)?;
                    debug!("[PYTH] ETH-USD: {} => {:?}", timestamp, value);
                }
                PYTH_SOL_FEED => {
                    self.sol.set(timestamp, value)?;
                    debug!("[PYTH] SOL-USD: {} => {:?}", timestamp, value);
                }
                _ => error!("[PYTH] Failed to find market for {}", price.id),
            }
        }

        Ok(())
    }

    // Need to be careful with rate limits
    pub async fn watch(&self, routine_interval: Option<u64>) {
        start_routine(std::cmp::max(1, routine_interval.unwrap_or(1)), || {
            Box::pin(async { self.fetch().await })
        }).await.unwrap();
    }
}
