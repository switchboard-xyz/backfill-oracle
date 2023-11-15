use anchor_lang::prelude::*;
use std::str::FromStr;
pub use anchor_lang::Discriminator;

declare_id!("3aiTRX5dhvWfgKa1kNwqF97jpGukMyphpj7UTcWmWfvV");

// Questions
// * Should we strictly rely on Anchor events? How do we backfill results?
// * Should we order transactions in a buffer and start flushing in order of timestamps?
//      This would let us fulfill multiple orders in the same txn

pub static BTC_MARKET_BYTES: [u8; 8] = [66, 84, 67, 0, 0, 0, 0, 0];
pub static ETH_MARKET_BYTES: [u8; 8] = [69, 84, 72, 0, 0, 0, 0, 0];
pub static SOL_MARKET_BYTES: [u8; 8] = [83, 79, 76, 0, 0, 0, 0, 0];

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Hash)]
pub enum MarketType {
    Btc,
    Eth,
    Sol,
}
impl MarketType {
    pub fn to_bytes(&self) -> [u8; 8] {
        match &self {
            MarketType::Btc => BTC_MARKET_BYTES,
            MarketType::Eth => ETH_MARKET_BYTES,
            MarketType::Sol => SOL_MARKET_BYTES,
        }
    }
}
impl FromStr for MarketType {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "BTC" => Ok(MarketType::Btc),
            "ETH" => Ok(MarketType::Eth),
            "SOL" => Ok(MarketType::Sol),
            _ => Err(error!(ProgramError::InvalidMarketName)),
        }
    }
}
impl TryFrom<[u8; 8]> for MarketType {
    type Error = Error;

    fn try_from(bytes: [u8; 8]) -> std::result::Result<Self, Self::Error> {
        if bytes == BTC_MARKET_BYTES {
            Ok(MarketType::Btc)
        } else if bytes == ETH_MARKET_BYTES {
            Ok(MarketType::Eth)
        } else if bytes == SOL_MARKET_BYTES {
            Ok(MarketType::Sol)
        } else {
            Err(error!(ProgramError::InvalidMarketName))
        }
    }
}

#[program]
pub mod backfill_oracle_program {
    use super::*;

    /// Create all of the program/oracle accounts
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.program.bump = ctx.bumps.program;
        ctx.accounts.program.authority = ctx.accounts.authority.key();

        let btc_market_name: [u8; 8] = BTC_MARKET_BYTES;
        ctx.accounts.program.markets.push(btc_market_name);
        ctx.accounts.btc_market.bump = ctx.bumps.btc_market;
        ctx.accounts.btc_market.name = btc_market_name;
        ctx.accounts.btc_market.decimals = 9;
        ctx.accounts.btc_market.oracle_staleness_threshold = 30;

        let eth_market_name: [u8; 8] = ETH_MARKET_BYTES;
        ctx.accounts.program.markets.push(eth_market_name);
        ctx.accounts.eth_market.bump = ctx.bumps.eth_market;
        ctx.accounts.eth_market.name = eth_market_name;
        ctx.accounts.eth_market.decimals = 9;
        ctx.accounts.eth_market.oracle_staleness_threshold = 30;

        let sol_market_name: [u8; 8] = SOL_MARKET_BYTES;
        ctx.accounts.program.markets.push(sol_market_name);
        ctx.accounts.sol_market.bump = ctx.bumps.sol_market;
        ctx.accounts.sol_market.name = sol_market_name;
        ctx.accounts.sol_market.decimals = 9;
        ctx.accounts.sol_market.oracle_staleness_threshold = 30;

        Ok(())
    }

    /// Create a dummy oracle and register the secure signer
    pub fn register_oracle(ctx: Context<RegisterOracle>) -> Result<()> {
        if ctx.accounts.oracle.bump == 0 {
            ctx.accounts.program.oracle = ctx.accounts.oracle.key();

            ctx.accounts.oracle.bump = ctx.bumps.oracle;
            ctx.accounts.oracle.authority = ctx.accounts.authority.key();
            ctx.accounts.oracle.verification_timestamp = Clock::get()?.unix_timestamp;
            ctx.accounts.oracle.verification_slot = Clock::get()?.slot;
            ctx.accounts.oracle.valid_until_slot = u64::MAX;
        }

        ctx.accounts.oracle.enclave_signer = ctx.accounts.enclave_signer.key();

        Ok(())
    }

    /// Create an order, read the oracle price, and emit an event if the price is stale
    pub fn create_order(ctx: Context<CreateOrder>, params: CreateOrderParams) -> Result<()> {
        ctx.accounts.order.open_order = 1;
        ctx.accounts.order.authority = ctx.accounts.authority.key();
        ctx.accounts.order.market = ctx.accounts.market.key();
        ctx.accounts.order.market_name = params.market.to_bytes();
        ctx.accounts.order.open_timestamp = Clock::get()?.unix_timestamp;
        ctx.accounts.order.open_slot = Clock::get()?.slot;

        emit!(OraclePriceRequestedEvent {
            oracle: ctx.accounts.program.oracle,
            order: ctx.accounts.order.key(),
            market: params.market,
            timestamp: ctx.accounts.order.open_timestamp,
            slot: ctx.accounts.order.open_slot,
        });

        Ok(())
    }

    /// Fulfill an order with a backfilled price
    pub fn fulfill_order(ctx: Context<FulfillOrder>, params: FulfillOrderParams) -> Result<()> {
        ctx.accounts.order.open_order = 0;
        ctx.accounts.order.close_timestamp = Clock::get()?.unix_timestamp;
        ctx.accounts.order.close_slot = Clock::get()?.slot;
        ctx.accounts.order.oracle_price = params.price;

        emit!(OraclePriceFulfilledEvent {
            market: params.market,
            order: ctx.accounts.order.key(),

            open_timestamp: ctx.accounts.order.open_timestamp,
            open_slot: ctx.accounts.order.open_slot,

            latency_seconds: ctx.accounts.order.close_timestamp - ctx.accounts.order.open_timestamp,
            latency_slots: ctx.accounts.order.close_slot - ctx.accounts.order.open_slot,

            price: params.price,
            decimals: ctx.accounts.market.decimals,
        });

        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct ProgramAccount {
    pub bump: u8,
    pub authority: Pubkey,
    pub oracle: Pubkey,
    #[max_len(16)]
    pub markets: Vec<[u8; 8]>,
}

#[account]
#[derive(InitSpace)]
pub struct MarketAccount {
    pub bump: u8,
    pub name: [u8; 8],

    // market configs
    pub decimals: u32,
    pub oracle_staleness_threshold: u32,

    // we could store & pop open orders here per market
}

#[account]
#[derive(InitSpace)]
pub struct OracleAccount {
    pub bump: u8,
    pub authority: Pubkey,
    pub enclave_signer: Pubkey,
    pub verification_timestamp: i64,
    pub verification_slot: u64,
    pub valid_until_slot: u64,
}

#[account]
#[derive(InitSpace)]
pub struct OrderAccount {
    // Flags for gPA filtering
    // 0 = false, 1 = true
    pub open_order: u8,
    pub reserved: [u8; 31],
    pub authority: Pubkey,
    pub market: Pubkey,
    pub market_name: [u8; 8],
    pub open_timestamp: i64,
    pub open_slot: u64,
    pub close_timestamp: i64,
    pub close_slot: u64,
    pub oracle_price: u64,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + ProgramAccount::INIT_SPACE,
        seeds = [b"PROGRAM"],
        bump
    )]
    pub program: Account<'info, ProgramAccount>,

    #[account(
        init,
        payer = payer,
        space = 8 + MarketAccount::INIT_SPACE,
        seeds = [program.key().to_bytes().as_ref(), b"BTC\0\0\0\0\0"],
        bump
    )]
    pub btc_market: Account<'info, MarketAccount>,

    #[account(
        init,
        payer = payer,
        space = 8 + MarketAccount::INIT_SPACE,
        seeds = [program.key().to_bytes().as_ref(), b"ETH\0\0\0\0\0"],
        bump
    )]
    pub eth_market: Account<'info, MarketAccount>,

    #[account(
        init,
        payer = payer,
        space = 8 + MarketAccount::INIT_SPACE,
        seeds = [program.key().to_bytes().as_ref(), b"SOL\0\0\0\0\0"],
        bump
    )]
    pub sol_market: Account<'info, MarketAccount>,

    /// CHECK:
    pub authority: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterOracle<'info> {
    #[account(
        mut,
        seeds = [b"PROGRAM"],
        bump = program.bump,
        // has_one = oracle,
    )]
    pub program: Account<'info, ProgramAccount>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + OracleAccount::INIT_SPACE,
        seeds = [b"ORACLE", authority.key().to_bytes().as_ref()],
        bump
    )]
    pub oracle: Account<'info, OracleAccount>,

    pub enclave_signer: Signer<'info>,

    /// CHECK:
    pub authority: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct CreateOrderParams {
    pub market: MarketType,
}

#[derive(Accounts)]
#[instruction(params: CreateOrderParams)] // rpc parameters hint
pub struct CreateOrder<'info> {
    #[account(init, payer = payer, space = 8 + OrderAccount::INIT_SPACE)]
    pub order: Account<'info, OrderAccount>,

    #[account(
        seeds = [b"PROGRAM"],
        bump = program.bump,
        constraint = program.oracle != Pubkey::default() @ ProgramError::OracleAlreadyRegistered
    )]
    pub program: Account<'info, ProgramAccount>,

    #[account(
        seeds = [program.key().to_bytes().as_ref(), params.market.to_bytes().as_ref()],
        bump = market.bump
    )]
    pub market: Account<'info, MarketAccount>,

    /// CHECK:
    pub authority: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FulfillOrderParams {
    pub market: MarketType,
    pub price: u64,
}

#[derive(Accounts)]
#[instruction(params: FulfillOrderParams)] // rpc parameters hint
pub struct FulfillOrder<'info> {
    #[account(
        mut,
        constraint = order.open_order == 1 @ ProgramError::OrderAlreadyFulfilled,
        has_one = market,
    )]
    pub order: Account<'info, OrderAccount>,

    #[account(seeds = [b"PROGRAM"], bump = program.bump, has_one = oracle)]
    pub program: Account<'info, ProgramAccount>,

    #[account(
        seeds = [program.key().to_bytes().as_ref(), params.market.to_bytes().as_ref()],
        bump = market.bump
    )]
    pub market: Account<'info, MarketAccount>,

    #[account(has_one = enclave_signer)]
    pub oracle: Account<'info, OracleAccount>,

    pub enclave_signer: Signer<'info>,
}

#[event]
#[derive(Debug)]
pub struct OraclePriceRequestedEvent {
    pub market: MarketType,
    pub oracle: Pubkey,
    pub order: Pubkey,
    pub timestamp: i64,
    pub slot: u64,
}

#[event]
#[derive(Debug)]
pub struct OraclePriceFulfilledEvent {
    pub market: MarketType,
    pub order: Pubkey,
    pub open_timestamp: i64,
    pub open_slot: u64,
    pub latency_seconds: i64,
    pub latency_slots: u64,
    pub price: u64,
    pub decimals: u32,
}

#[error_code]
#[derive(Eq, PartialEq)]
pub enum ProgramError {
    OracleAlreadyRegistered,
    OrderAlreadyFulfilled,
    InvalidMarketName,
}
