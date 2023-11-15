use crate::*;

use base64::{ engine::general_purpose, Engine as _ };
use switchboard_solana::{
    solana_client::{
        nonblocking::pubsub_client::PubsubClient,
        rpc_config::{
            RpcTransactionLogsFilter,
            RpcTransactionLogsConfig,
            RpcProgramAccountsConfig,
            RpcAccountInfoConfig,
        },
    },
    solana_sdk::commitment_config::CommitmentConfig,
    get_ixn_discriminator,
};
use std::ops::Deref;
use solana_program::hash::Hash;
use anchor_lang::Discriminator;
use futures::future::join_all;
use std::time::Duration;

pub struct OracleWorker {
    pub status: WorkerStatus,

    pub client: Arc<RwLock<Client<Arc<Keypair>>>>,
    pub program: Arc<Program<Arc<Keypair>>>,
    pub program_id: Pubkey,
    pub rpc: Arc<RpcClient>,
    pub pubsub_client: Arc<PubsubClient>,
    pub payer: Arc<Keypair>,
    pub payer_pubkey: Pubkey,

    pub program_state_pubkey: Pubkey,
    pub oracle_pubkey: Pubkey,
    pub enclave_signer: Arc<Keypair>,

    pub payer_balance: Arc<RwLock<u64>>,
    pub recent_blockhash: Arc<RwLock<Hash>>,
    pub slot: Arc<RwLock<u64>>,

    pub active_orders: Arc<DashSet<Pubkey>>,
    pub markets: Arc<DashMap<MarketType, Pubkey>>,

    pub coinbase: CoinbaseProvider,
    pub pyth: PythProvider,
}

impl OracleWorker {
    pub async fn run() -> Result<(), SbError> {
        let mut worker = OracleWorker::new().await?;
        worker.initialize().await?;

        worker.start().await?;

        Err(SbError::Message("Switchboard worker crashed!"))
    }
    /// Initialize a new Solana worker with a cache.
    pub async fn new() -> Result<Self, SbError> {
        println!(">>>>>>>> Creating worker <<<<<<<<");

        // Parse environment variables
        let env = WorkerEnvironment::get_or_init();
        // println!("{:#?}", env);

        let payer = env.get_payer().unwrap();
        let payer_pubkey = payer.pubkey();
        println!("Payer: {:?}", payer_pubkey);

        let enclave_signer = env.load_enclave_signer(None)?;

        let client = anchor_client::Client::new_with_options(
            anchor_client::Cluster::from_str(env.rpc_url.as_str()).unwrap_or(Cluster::Devnet),
            payer.clone(),
            solana_sdk::commitment_config::CommitmentConfig::processed()
        );

        let program_id = env.get_program_id();

        let program: Arc<Program<Arc<Keypair>>> = Arc::new(client.program(program_id).unwrap());
        println!("ProgramID: {:?}", program_id);

        let ws_url = program
            .clone()
            .async_rpc()
            .url()
            .replace("http://", "ws://")
            .replace("https://", "wss://");
        let pubsub_client = PubsubClient::new(ws_url.as_str()).await.unwrap();

        let (program_state_pubkey, _) = Pubkey::find_program_address(&[b"PROGRAM"], &program_id);

        let (oracle_pubkey, _) = Pubkey::find_program_address(
            &[b"ORACLE", payer_pubkey.to_bytes().as_ref()],
            &program_id
        );

        let markets: DashMap<MarketType, Pubkey> = vec![
            (
                MarketType::Btc,
                Pubkey::find_program_address(
                    &[program_state_pubkey.to_bytes().as_ref(), b"BTC\0\0\0\0\0"],
                    &program_id
                ).0,
            ),
            (
                MarketType::Eth,
                Pubkey::find_program_address(
                    &[program_state_pubkey.to_bytes().as_ref(), b"ETH\0\0\0\0\0"],
                    &program_id
                ).0,
            ),
            (
                MarketType::Sol,
                Pubkey::find_program_address(
                    &[program_state_pubkey.to_bytes().as_ref(), b"SOL\0\0\0\0\0"],
                    &program_id
                ).0,
            )
        ]
            .into_iter()
            .collect();

        Ok(Self {
            status: WorkerStatus::Initializing,

            client: Arc::new(RwLock::new(client)),
            program: program.clone(),
            program_id,
            rpc: Arc::new(program.clone().async_rpc()),
            pubsub_client: Arc::new(pubsub_client),
            program_state_pubkey,
            oracle_pubkey,
            enclave_signer,
            payer,
            payer_pubkey,

            payer_balance: Default::default(),
            recent_blockhash: Default::default(),
            slot: Default::default(),

            active_orders: Arc::new(DashSet::new()),
            markets: Arc::new(markets),

            coinbase: Default::default(),
            pyth: Default::default(),
        })
    }

    // Initialize the oracle worker
    // * Start populating cache
    // * Call register_oracle
    // * Start metrics and healthcheck
    pub async fn initialize(&mut self) -> Result<(), SbError> {
        println!(">>>>>>>> Initializing worker <<<<<<<<");

        self.initialize_program_accounts().await?;
        self.initialize_oracle_signer().await?;

        self.status = WorkerStatus::Ready;

        Ok(())
    }

    pub async fn start(&mut self) -> Result<(), SbError> {
        println!(">>>>>>>> Starting worker <<<<<<<<");

        tokio::select! {
            // Watch on-chain data to speed up txn building
            _ = self.watch_blockhash_and_slot(None) => {
                 Err(SbError::Message("watch_blockhash_and_slot returned unexpectedly"))
                // panic!("watch_blockhash_and_slot returned unexpectedly");
            }
            _ = self.watch_payer_balance(None)=> {
                 Err(SbError::Message("watch_payer_balance returned unexpectedly"))
                // panic!("watch_payer_balance returned unexpectedly");
            }

            // Watch on-chain events to respond to stale oracle prices
            _ = self.watch_anchor_events() => {
                 Err(SbError::Message("watch_anchor_events returned unexpectedly"))
                // panic!("watch_anchor_events returned unexpectedly");
            }
            _ = self.watch_open_order_accounts(None) => {
                 Err(SbError::Message("watch_open_order_accounts returned unexpectedly"))
                // panic!("watch_open_order_accounts returned unexpectedly");
            }

            // Watch data sources so our cache is fresh
            _ = self.coinbase.watch() => {
                 Err(SbError::Message("watch_coinbase returned unexpectedly"))
                // panic!("watch_coinbase returned unexpectedly");
            }
            _ = self.pyth.watch(None) => {
                 Err(SbError::Message("watch_pyth returned unexpectedly"))
                // panic!("watch_pyth returned unexpectedly");
            }
        }
    }
    async fn initialize_program_accounts(&self) -> Result<(), SbError> {
        match self.rpc.get_account(&self.program_state_pubkey).await {
            Ok(_account) => Ok(()),
            Err(e) => {
                println!("Program accounts not initialized: {:?}", e);
                Err(SbError::Message("Program accounts not initialized"))
            }
        }
    }

    async fn initialize_oracle_signer(&self) -> Result<(), SbError> {
        let enclave_signer = self.enclave_signer.clone();
        let enclave_signer_pubkey = enclave_signer.pubkey();

        if let Ok(account) = self.rpc.get_account(&self.oracle_pubkey).await {
            if let Ok(oracle_account) = OracleAccount::deserialize(&mut account.data.as_slice()) {
                if oracle_account.enclave_signer == enclave_signer_pubkey {
                    println!("Enclave signer already set, skipping");
                    return Ok(());
                }
            } else {
                error!("Failed to deserialize oracle account");
            }
        }

        let signers = vec![self.payer.as_ref(), enclave_signer.deref()];

        let msg = Message::new(
            &[
                Instruction {
                    program_id: self.program_id,
                    accounts: vec![
                        AccountMeta::new(self.program_state_pubkey, false),
                        AccountMeta::new(self.oracle_pubkey, false),
                        AccountMeta::new_readonly(enclave_signer_pubkey, true),
                        AccountMeta::new_readonly(self.payer_pubkey, true),
                        AccountMeta::new(self.payer_pubkey, true),
                        AccountMeta::new_readonly(solana_program::system_program::ID, false)
                    ],
                    data: get_ixn_discriminator("register_oracle").to_vec(),
                },
            ],
            Some(&self.payer_pubkey)
        );
        let mut tx = Transaction::new_unsigned(msg);

        let blockhash = self.rpc.get_latest_blockhash().await.unwrap_or_default();

        tx.try_sign(&signers, blockhash).map_err(|e| SbError::CustomError {
            message: "Failed to sign txn".into(),
            source: std::sync::Arc::new(e),
        })?;

        let signature = self.rpc
            .send_and_confirm_transaction(&tx).await
            .map_err(|e| SbError::CustomError {
                message: "Failed to send txn".into(),
                source: std::sync::Arc::new(e),
            })?;
        info!("[ORACLE] initialized: {}", signature);

        Ok(())
    }

    /// Periodically fetch the Solana time from on-chain so we know when to execute functions.
    async fn watch_blockhash_and_slot(&self, routine_interval: Option<u64>) {
        start_routine(std::cmp::max(1, routine_interval.unwrap_or(1)), || {
            Box::pin(async {
                self.fetch_blockhash_and_slot().await;

                Ok(())
            })
        }).await.unwrap();
    }

    async fn fetch_blockhash_and_slot(&self) {
        let blockhash_result = tokio::join!(
            self.rpc.get_latest_blockhash_with_commitment(CommitmentConfig::processed())
        );

        if let Ok((blockhash, slot)) = blockhash_result.0 {
            let mut recent_blockhash = self.recent_blockhash.write().await;
            *recent_blockhash = blockhash;

            let mut last_valid_block_height: tokio::sync::RwLockWriteGuard<
                '_,
                u64
            > = self.slot.write().await;
            *last_valid_block_height = slot;
        }
    }

    /// Periodically fetch the Solana time from on-chain so we know when to execute functions.
    async fn watch_payer_balance(&self, routine_interval: Option<u64>) {
        start_routine(std::cmp::max(5, routine_interval.unwrap_or(30)), || {
            Box::pin(async {
                self.fetch_payer_balance().await;

                Ok(())
            })
        }).await.unwrap();
    }

    async fn fetch_payer_balance(&self) {
        match self.rpc.get_balance(&self.payer_pubkey).await {
            Ok(balance) => {
                let payer_balance_decimal = SwitchboardDecimal {
                    mantissa: balance.try_into().unwrap(),
                    scale: 9,
                };
                let payer_decimal_float: f64 = payer_balance_decimal.try_into().unwrap();
                info!("PAYER_BALANCE: {:?} SOL", payer_decimal_float);

                if balance <= 10000 {
                    error!(
                        "Payer ({}) balance is low on funds {:?}",
                        self.payer_pubkey,
                        payer_decimal_float
                    );
                    panic!(
                        "Payer ({}) balance is low on funds {:?}",
                        self.payer_pubkey,
                        payer_decimal_float
                    );
                }

                let mut payer_balance = self.payer_balance.write().await;
                *payer_balance = balance;
            }
            Err(e) => error!("Failed to fetch payer balance: {:?}", e),
        }
    }

    /// Call getProgramAccounts and find open orders that are ready to be executed.
    async fn watch_open_order_accounts(&self, routine_interval: Option<u64>) {
        start_routine(std::cmp::max(1, routine_interval.unwrap_or(1)), || {
            Box::pin(async {
                let open_orders = self.fetch_open_order_accounts().await.unwrap_or_default();
                if open_orders.is_empty() {
                    return Ok(());
                }

                info!("Found {} open orders to fulfill", open_orders.len());

                // Vector to store futures
                let mut futures = Vec::new();
                let mut order_keys = Vec::new();

                for (order_key, order_data) in open_orders {
                    if self.active_orders.insert(order_key) {
                        info!("[ORDER] Found open order {}", order_key);

                        let future = self.fulfill_order(
                            order_key,
                            MarketType::try_from(order_data.market_name).unwrap(),
                            order_data.open_timestamp
                        );
                        futures.push(future);
                        order_keys.push(order_key);
                    }
                }

                // Wait for all futures to complete
                let results = join_all(futures).await;

                // Process the results
                for (i, result) in results.iter().enumerate() {
                    match result {
                        Ok(_) => {
                            if let Some(order_key) = order_keys.get(i) {
                                info!("[ORDER] order fulfilled {:?}", order_key);
                            } else {
                                info!("[ORDER] order fulfilled");
                            }
                        }
                        Err(e) => {
                            error!("[ORDER] order failed: {:?}", e);
                            if let Some(order_key) = order_keys.get(i) {
                                self.active_orders.remove(order_key);
                            } else {
                                error!("Failed to find order key");
                            }
                        }
                    }
                }

                Ok(())
            })
        }).await.unwrap();
    }
    /// Fetch all of the open orders based on the 8-byte discriminator and the open_order flag
    async fn fetch_open_order_accounts(&self) -> Result<Vec<(Pubkey, OrderAccount)>, SbError> {
        let mut open_orders: Vec<(Pubkey, OrderAccount)> = vec![];

        let mut order_account_discriminator_filter = OrderAccount::discriminator().to_vec();
        order_account_discriminator_filter.push(1);

        let accounts = self.rpc
            .get_program_accounts_with_config(&self.program_id, RpcProgramAccountsConfig {
                filters: Some(
                    vec![
                        solana_client::rpc_filter::RpcFilterType::Memcmp(
                            solana_client::rpc_filter::Memcmp::new_raw_bytes(
                                0,
                                order_account_discriminator_filter
                            )
                        )
                    ]
                ),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(solana_account_decoder::UiAccountEncoding::Base64Zstd),
                    ..Default::default()
                },
                ..Default::default()
            }).await
            .map_err(|e| SbError::CustomError {
                message: "Failed to get program accounts".to_string(),
                source: Arc::new(e),
            })?;

        for (pubkey, account) in accounts {
            if let Ok(order_data) = OrderAccount::try_deserialize(&mut &account.data[..]) {
                open_orders.push((pubkey, order_data));
            }
        }

        Ok(open_orders)
    }

    /// Stream websocket events for the request trigger event
    async fn watch_anchor_events(&self) {
        let mut retry_count = 0;
        let max_retries = 3;
        let mut delay = Duration::from_millis(500); // start with a 500ms delay

        loop {
            // Attempt to connect
            let connection_result = self.pubsub_client.logs_subscribe(
                RpcTransactionLogsFilter::Mentions(vec![self.program_id.to_string()]),
                RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::processed()),
                }
            ).await;

            match connection_result {
                Ok((mut stream, _handler)) => {
                    retry_count = 0; // Reset retry count on successful connection
                    delay = Duration::from_millis(500); // Reset delay on successful connection

                    // Process events if connection is successful

                    while let Some(event) = stream.next().await {
                        let log: String = event.value.logs.join(" ");
                        for w in log.split(' ') {
                            let decoded = general_purpose::STANDARD.decode(w);
                            if decoded.is_err() {
                                continue;
                            }
                            let decoded = decoded.unwrap();
                            if decoded.len() < 8 {
                                continue;
                            }

                            if decoded[..8] == OraclePriceRequestedEvent::DISCRIMINATOR {
                                if
                                    let Ok(event) = OraclePriceRequestedEvent::try_from_slice(
                                        &decoded[8..]
                                    )
                                {
                                    self.handle_price_request_event(event).await;
                                }
                            } else if decoded[..8] == OraclePriceFulfilledEvent::DISCRIMINATOR {
                                if
                                    let Ok(event) = OraclePriceFulfilledEvent::try_from_slice(
                                        &decoded[8..]
                                    )
                                {
                                    self.handle_order_fulfilled_event(event).await;
                                }
                            }

                            continue;
                        }
                    }

                    error!("[WEBSOCKET] connection closed, attempting to reconnect...");
                }
                Err(e) => {
                    error!("[WEBSOCKET] Failed to connect: {:?}", e);
                    if retry_count >= max_retries {
                        error!("[WEBSOCKET] Maximum retry attempts reached, aborting...");
                        break;
                    }

                    tokio::time::sleep(delay).await; // wait before retrying
                    retry_count += 1;
                    delay = std::cmp::min(delay * 2, Duration::from_secs(5)); // Double the delay for next retry, up to 5 seconds
                }
            }

            if retry_count >= max_retries {
                error!("[WEBSOCKET] Maximum retry attempts reached, aborting...");
                break;
            }
        }
    }

    async fn handle_price_request_event(&self, event: OraclePriceRequestedEvent) {
        println!("[OraclePriceRequestedEvent] {:#?}", event);

        if self.active_orders.insert(event.order) {
            match self.fulfill_order(event.order, event.market, event.timestamp).await {
                Ok(_) => {
                    info!("[ORDER] order fulfilled");
                    // TODO: should we remove from the map after some delay so it doesnt get processed twice?
                }
                Err(e) => {
                    error!("[ORDER] order failed: {:?}", e);
                    self.active_orders.remove(&event.order);
                }
            }
        }
    }

    // TODO: check the cache and see if this is a newly emitted event
    async fn handle_order_fulfilled_event(&self, event: OraclePriceFulfilledEvent) {
        println!("[OraclePriceFulfilledEvent] {:#?}", event);
    }

    async fn get_price(&self, market: &MarketType, timestamp: i64) -> Result<u64, SbError> {
        let (pyth, coinbase) = match market {
            MarketType::Btc => (&self.pyth.btc, &self.coinbase.btc),
            MarketType::Eth => (&self.pyth.eth, &self.coinbase.eth),
            MarketType::Sol => (&self.pyth.sol, &self.coinbase.sol),
        };

        // Always require the pyth provider to return a price
        let pyth_price = pyth.get(timestamp).await?;

        // If the coinbase websocket has provided a price, use the average price of pyth + coinbase
        // This is for demo purposes only - three or more sources should be combined with a median
        let price = if let Some(coinbase_price) = coinbase.get(&timestamp) {
            coinbase_price.checked_add(pyth_price).unwrap().checked_div(2).unwrap()
        } else {
            pyth_price
        };

        Ok(price)
    }

    // Here we can wait and group ixns if we need to
    async fn fulfill_order(
        &self,
        order_pubkey: Pubkey,
        market: MarketType,
        timestamp: i64
    ) -> Result<(), SbError> {
        let price = self.get_price(&market, timestamp).await?;
        let market_pubkey = *self.markets.get(&market).unwrap();

        let mut ixn_data = get_ixn_discriminator("fulfill_order").to_vec();
        let ixn_params = FulfillOrderParams {
            market,
            price,
        };
        ixn_data.append(&mut ixn_params.try_to_vec().unwrap());

        let enclave_signer = self.enclave_signer.clone();
        let enclave_signer_pubkey = enclave_signer.pubkey();
        let signers = vec![self.payer.as_ref(), enclave_signer.deref()];

        let msg = Message::new(
            &[
                Instruction {
                    program_id: self.program_id,
                    accounts: vec![
                        AccountMeta::new(order_pubkey, false),
                        AccountMeta::new_readonly(self.program_state_pubkey, false),
                        AccountMeta::new_readonly(market_pubkey, false),
                        AccountMeta::new(self.oracle_pubkey, false),
                        AccountMeta::new_readonly(enclave_signer_pubkey, true)
                    ],
                    data: ixn_data,
                },
            ],
            Some(&self.payer_pubkey)
        );
        let mut tx = Transaction::new_unsigned(msg);

        tx
            .try_sign(&signers, *self.recent_blockhash.read().await)
            .map_err(|e| SbError::CustomError {
                message: "Failed to sign txn".into(),
                source: std::sync::Arc::new(e),
            })?;

        let signature = self.rpc
            .send_and_confirm_transaction(&tx).await
            .map_err(|e| SbError::CustomError {
                message: "Failed to send txn".into(),
                source: std::sync::Arc::new(e),
            })?;

        info!("[ORACLE] fulfill_order: {}", signature);

        Ok(())
    }
}
