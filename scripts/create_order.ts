import type { BackfillOracleProgram } from "../target/types/backfill_oracle_program";

import { loadKeypair, loadMarkets, loadProgram } from "./utils";

import * as anchor from "@coral-xyz/anchor";
import chalk from "chalk";
import dotenv from "dotenv";
dotenv.config();

(async () => {
  console.log(
    `\n${chalk.green(
      "This script will create an order for our backfill oracle program."
    )}`
  );

  const [program, payer] = loadProgram();

  const [programPubkey, btcMarket, ethMarket, solMarket] = loadMarkets(program);

  // Use the random number in a switch statement
  let market: any = { btc: {} };
  let marketString = "BTC";
  let marketPubkey = btcMarket;
  switch (Math.floor(Math.random() * 3)) {
    case 0:
      market = { btc: {} };
      marketString = "BTC";
      marketPubkey = btcMarket;
      break;
    case 1:
      market = { eth: {} };
      marketString = "ETH";
      marketPubkey = ethMarket;
      break;
    case 2:
      market = { sol: {} };
      marketString = "SOL";
      marketPubkey = solMarket;
      break;
    default:
      market = { btc: {} };
      marketString = "BTC";
      marketPubkey = btcMarket;
  }

  const keypair = anchor.web3.Keypair.generate();

  const txn = await program.methods
    .createOrder({ market })
    .accounts({
      order: keypair.publicKey,
      program: programPubkey,
      market: marketPubkey,
      authority: payer.publicKey,
      payer: payer.publicKey,
    })
    .signers([keypair])
    .rpc();
  console.log(`[TX] create_order (${marketString}): ${txn}`);
})();
