pub use switchboard_solana::prelude::*;
pub use solana_sdk::signer::Signer;
pub use kv_log_macro::{ info, debug, trace, error };

pub mod types;
pub use types::*;

pub mod worker;
pub use worker::*;

pub mod cache;
pub use cache::*;

pub mod providers;
pub use providers::*;

pub mod env;
pub use env::*;

pub mod utils;
pub use utils::*;

pub use futures::{ Future, StreamExt };
pub use std::sync::Arc;
pub use std::str::FromStr;
pub use std::io::Error;
pub use tokio::sync::RwLock;
pub use anchor_client::Client;
pub use switchboard_solana::solana_client::nonblocking::rpc_client::RpcClient;
pub use anchor_client::Program;

pub use dashmap::{ DashMap, DashSet };

pub use backfill_oracle_program::{
    ID as ProgramID,
    MarketAccount,
    ProgramAccount,
    OracleAccount,
    OrderAccount,
    OraclePriceFulfilledEvent,
    OraclePriceRequestedEvent,
    MarketType,
    RegisterOracle,
    FulfillOrderParams,
};

pub use miette::Result;
use tokio_graceful_shutdown::{ SubsystemBuilder, SubsystemHandle, Toplevel };

// Execution Flow
// * Read env variables to determine program ID to target. And payer keypair as env variable (meh)
// * Generate enclave signer and send register_oracle ixn
// * Start building a TTL cache of timestamp/slot to price for each of the 5 assets
// * Subscribe to Anchor event BackfillOracleData
// * Periodically fetch all order accounts and find orders that need to be backfilled

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // Set up logging
    femme::with_level(
        femme::LevelFilter
            ::from_str(
                std::env::var("RUST_LOG").unwrap_or("info".to_owned()).to_ascii_lowercase().as_str()
            )
            .unwrap_or(femme::LevelFilter::Info)
    );

    // Setup and execute subsystem tree
    Toplevel::new(|handle| async move {
        handle.start(
            SubsystemBuilder::new("Switchboard Worker", |subsys: SubsystemHandle| async move {
                // start_oracle

                tokio::select! {
                _ = subsys.on_shutdown_requested() => {
                    info!("Shutdown requested.");
                },
                _ = OracleWorker::run() => {
                    subsys.request_shutdown();
                }
            }

                Ok::<(), SbError>(())
            })
        );
    })
        .catch_signals()
        .handle_shutdown_requests(tokio::time::Duration::from_millis(1000)).await
        .map_err(Into::into)

    // let mut worker = OracleWorker::new().await?;
    // worker.initialize().await?;

    // worker.start().await;

    // panic!("Switchboard worker crashed!");
}
