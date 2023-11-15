use crate::*;
use r_cache::cache::Cache;
use serde::{ Serialize, Deserialize };
use chrono::{ DateTime, Utc };
use std::time::Duration;
use futures_util::SinkExt;
use tokio_tungstenite::{ connect_async, tungstenite::protocol::Message };
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Debug)]
pub struct CoinbaseTickerMessage {
    #[serde(rename = "type")]
    pub type_: String,
    pub sequence: u64,
    pub product_id: String,
    pub price: String,
    pub open_24h: String,
    pub volume_24h: String,
    pub low_24h: String,
    pub high_24h: String,
    pub volume_30d: String,
    pub best_bid: String,
    pub best_bid_size: String,
    pub best_ask: String,
    pub best_ask_size: String,
    pub side: String,
    pub time: DateTime<Utc>,
    pub trade_id: u64,
    pub last_size: String,
}
impl CoinbaseTickerMessage {
    pub fn to_f64_price(&self) -> Result<f64, SbError> {
        match self.price.parse::<f64>() {
            Ok(price) => Ok(price),
            Err(_e) => Err(SbError::Message("Failed to convert Coinbase price to f64")),
        }
    }

    pub fn to_u64_price(&self) -> Result<u64, SbError> {
        let value = self.to_f64_price()?;

        if value.is_nan() || value.is_infinite() || value < 0.0 {
            Err(SbError::Message("Invalid input"))
        } else {
            let multiplied = value * 1_000_000_000_f64;
            if multiplied > (u64::MAX as f64) {
                Err(SbError::Message("Overflow occured"))
            } else {
                Ok(multiplied as u64)
            }
        }
    }
}

#[derive(Clone)]
pub struct CoinbaseProvider {
    pub btc: Arc<Cache<i64, u64>>,
    pub eth: Arc<Cache<i64, u64>>,
    pub sol: Arc<Cache<i64, u64>>,
}
impl std::default::Default for CoinbaseProvider {
    fn default() -> Self {
        Self {
            btc: Arc::new(Cache::new(Some(Duration::from_secs(3600)))),
            eth: Arc::new(Cache::new(Some(Duration::from_secs(3600)))),
            sol: Arc::new(Cache::new(Some(Duration::from_secs(3600)))),
        }
    }
}
impl CoinbaseProvider {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn watch(&self) {
        let (mut ws_stream, _) = connect_async("wss://ws-feed.exchange.coinbase.com").await.expect(
            "Failed to connect"
        );

        let subscribe_message = Message::Text(
            "{\"type\":\"subscribe\",\"product_ids\":[\"BTC-USD\"],\"channels\":[\"ticker\",{\"name\":\"ticker\",\"product_ids\":[\"BTC-USD\",\"ETH-USD\",\"SOL-USD\"]}]}".to_string()
        );

        ws_stream.send(subscribe_message).await.expect("Failed to send message");

        let (tx, mut rx) = mpsc::channel(1000);

        // Spawn a task to handle incoming messages
        tokio::spawn(async move {
            while let Some(message) = ws_stream.next().await {
                if let Ok(Message::Text(text)) = message {
                    tx.send(text).await.expect("Failed to send message");
                }
            }
        });

        // Temporary storage for averaging
        let btc_temp_store: Arc<Cache<i64, (u64, u32)>> = Arc::new(
            Cache::new(Some(Duration::from_secs(60)))
        );
        let eth_temp_store: Arc<Cache<i64, (u64, u32)>> = Arc::new(
            Cache::new(Some(Duration::from_secs(60)))
        );
        let sol_temp_store: Arc<Cache<i64, (u64, u32)>> = Arc::new(
            Cache::new(Some(Duration::from_secs(60)))
        );

        while let Some(data) = rx.recv().await {
            if let Ok(ticker) = serde_json::from_str::<CoinbaseTickerMessage>(&data) {
                let (cache, temp_store) = match ticker.product_id.as_str() {
                    "BTC-USD" => (self.btc.clone(), btc_temp_store.clone()),
                    "ETH-USD" => (self.eth.clone(), eth_temp_store.clone()),
                    "SOL-USD" => (self.sol.clone(), sol_temp_store.clone()),
                    _ => {
                        continue;
                    }
                };

                let timestamp = ticker.time.timestamp();

                match ticker.to_u64_price() {
                    Err(_e) => {
                        error!("[COINBASE] Failed to convert price to u64");
                        continue;
                    }
                    Ok(price) => {
                        let avg_price = if let Some(entry) = temp_store.get(&timestamp) {
                            let total_price = entry.0 + price;
                            let num_prices = entry.1 + 1;

                            temp_store.set(timestamp, (total_price, num_prices), None);
                            total_price / (num_prices as u64)
                        } else {
                            temp_store.set(timestamp, (price, 1), None);
                            price
                        };

                        cache.set(timestamp, avg_price, None);
                        debug!("[COINBASE] {}: {} => {}", ticker.product_id, timestamp, avg_price);
                    }
                };
            }
        }
    }
}
