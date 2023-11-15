import type { BackfillOracleProgram } from "../target/types/backfill_oracle_program";

import {
  getMarketNameBytes,
  loadKeypair,
  loadMarkets,
  loadProgram,
} from "./utils";

import type * as anchor from "@coral-xyz/anchor";
import chalk from "chalk";
import dotenv from "dotenv";
dotenv.config();

(async () => {
  console.log(
    `\n${chalk.green(
      "This script will initialize the backfill oracle programs state accounts so we can start placing orders."
    )}`
  );

  const [program, payer] = loadProgram();

  const [programPubkey, btcMarket, ethMarket, solMarket] = loadMarkets(program);

  await verifyAccountDoesntExist(
    program.provider.connection,
    programPubkey,
    "ProgramState"
  );
  await verifyAccountDoesntExist(
    program.provider.connection,
    btcMarket,
    "BTC Market"
  );
  await verifyAccountDoesntExist(
    program.provider.connection,
    ethMarket,
    "ETH Market"
  );
  await verifyAccountDoesntExist(
    program.provider.connection,
    solMarket,
    "SOL Market"
  );

  const txn = await program.methods
    .initialize()
    .accounts({
      program: programPubkey,
      btcMarket,
      ethMarket,
      solMarket,
      authority: payer.publicKey,
      payer: payer.publicKey,
    })
    .rpc();
  console.log(`[TX] initialize: ${txn}`);
})();

export async function verifyAccountDoesntExist(
  connection: anchor.web3.Connection,
  pubkey: anchor.web3.PublicKey,
  name: String
) {
  const accountInfo = await connection.getAccountInfo(pubkey);
  if (accountInfo && accountInfo.data) {
    throw new Error(`Account ${name} already initialized`);
  }
}
