# Backfill Oracle

Pyth prices on Solana can become stale during periods of congestion causing
transactions that rely on the data to fail. Dapps can integrate the Switchboard
backfill oracle to constantly watch the chain and fulfill orders where the Pyth
price was determined to be stale. This example also subscribes to a Coinbase
websocket for added redundancy.

Current Metrics:

| Metric            | Value                              |
| ----------------- | ---------------------------------- |
| Latency (seconds) | 5.65 seconds @ 7.8 requests/second |
| Latency (slots)   | 15.38 slots @ 7.8 requests/second  |

## Usage

First deploy the program to devnet:

```bash
anchor keys sync
anchor build
anchor deploy
anchor idl init -f target/idl/backfill_oracle_program.json $(solana-keygen pubkey target/deploy/backfill_oracle_program-keypair.json)
```

Then initialize the program accounts

```bash
anchor run init
```

Then create an order and emit the `OraclePriceRequestedEvent`:

```bash
anchor run create_order
```

We'll start the worker oracle in a new shell. First update the `.env` file:

```bash
cd switchboard-function
cp .env.sample .env
# Update the .env with your config and keypair
```

Then run the oracle worker

```bash
cargo run
```

You should now see the oracle responding to events and watching for open orders.
