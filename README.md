# Backfill Oracle

Pyth prices on Solana can become stale during periods of congestion causing
transactions that rely on the data to fail. Dapps can integrate the Switchboard
backfill oracle to constantly watch the chain and fulfill orders where the Pyth
price was determined to be stale. This example also subscribes to a Coinbase
websocket for added redundancy.

This is a proof of concept showing how a long running Switchboard Function can
be employed to manage your dApp and respond to on-chain events quickly. The
performance will depend on external factors like network resources that must be
fetched.

Switchboard Workers are still in development.

## Benchmark

The following are benchmarks collected on devnet while running the Switchboard
worker inside of an SGX enclave:

| Throughput  | Latency                    |
| ----------- | -------------------------- |
| 1.7 req/sec | 2.95 seconds (7.84 slots)  |
| 2.7 req/sec | 3.03 seconds (8.23 slots)  |
| 7.8 req/sec | 5.65 seconds (15.38 slots) |

**NOTE:** This implementation uses Pyth's Hermes REST endpoint. Using a
websocket to retrieve Pyth prices will provide better performance over polling.

## Usage

### Program Deploy

First, deploy the program to devnet:

```bash
anchor keys sync
anchor build
anchor deploy
anchor idl init -f target/idl/backfill_oracle_program.json $(solana-keygen pubkey target/deploy/backfill_oracle_program-keypair.json)
```

Then, initialize the program accounts:

```bash
anchor run init
```

Then, create an order and emit the `OraclePriceRequestedEvent`:

```bash
anchor run create_order
```

### Switchboard Worker

Finally, we'll start the worker oracle in a new shell. First update the `.env`
file:

```bash
cd switchboard-function
cp .env.sample .env
# Update the .env with your config and keypair
```

Then, run the oracle worker

```bash
cargo run
```

You should now see the oracle responding to events and watching for open orders.

### Benchmarking

To spam the worker, run the `./spam.sh` script in the root of this repository.
This will create and fund a set of keypairs on devnet and start creating orders.

Run the command `anchor run metrics` to read the program accounts and collect
some usage metrics.
