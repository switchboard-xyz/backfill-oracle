import type { BackfillOracleProgram } from "../target/types/backfill_oracle_program";

import { loadKeypair, loadMarkets, loadProgram } from "./utils";

import type * as anchor from "@coral-xyz/anchor";
import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import Big from "big.js";
import chalk from "chalk";
import dotenv from "dotenv";
import fs from "fs";
dotenv.config();

const ORDER_ACCOUNT_DISCRIMINATOR: string = "EFxNNV2jHAE";

interface IMetric {
  openTimestamp: number;
  closeTimestamp: number;
  latency: number;
  openSlot: number;
  closeSlot: number;
  slotLatency: number;
  oraclePrice: number;
  market: string;
}

(async () => {
  console.log(
    `\n${chalk.green(
      "This script will populate a CSV file to derive some metrics on the order accounts like average latency."
    )}`
  );

  const [program, payer] = loadProgram();
  const [programState, btcMarket, ethMarket, solMarket] = loadMarkets(program);

  const accounts = await program.provider.connection.getProgramAccounts(
    program.programId,
    { filters: [{ memcmp: { offset: 0, bytes: ORDER_ACCOUNT_DISCRIMINATOR } }] }
  );

  const metrics: IMetric[] = [];
  for (const { account, pubkey } of accounts) {
    const orderAccount: OrderAccount = program.coder.accounts.decode(
      "OrderAccount",
      account.data
    );
    if (orderAccount.openOrder !== 0) {
      continue;
    }

    const openTimestamp = orderAccount.openTimestamp.toNumber();
    const closeTimestamp = orderAccount.closeTimestamp.toNumber();
    const openSlot = orderAccount.openSlot.toNumber();
    const closeSlot = orderAccount.closeSlot.toNumber();
    const priceMantissa = new Big(orderAccount.oraclePrice.toString());
    const price = priceMantissa / new Big(1000000000);
    const market = orderAccount.market.equals(btcMarket)
      ? "BTC"
      : orderAccount.market.equals(ethMarket)
      ? "ETH"
      : orderAccount.market.equals(solMarket)
      ? "SOL"
      : "BTC";

    metrics.push({
      openTimestamp,
      closeTimestamp,
      latency: closeTimestamp - openTimestamp,
      openSlot,
      closeSlot,
      slotLatency: closeSlot - openSlot,
      oraclePrice: price,
      market,
    });
  }

  const averageLatency =
    metrics.reduce((acc, metric) => acc + metric.latency, 0) / metrics.length;
  const averageSlots =
    metrics.reduce((acc, metric) => acc + metric.slotLatency, 0) /
    metrics.length;

  const startingTimestamp = metrics.reduce(
    (min, metrics) => Math.min(min, metrics.openTimestamp),
    Number.MAX_SAFE_INTEGER
  );
  const endingTimestamp = metrics.reduce(
    (min, metrics) => Math.max(min, metrics.closeTimestamp),
    0
  );
  const maxLatency = metrics.reduce(
    (max, metrics) => Math.max(max, metrics.latency),
    0
  );
  const minLatency = metrics.reduce(
    (min, metrics) =>
      metrics.latency === 0 ? min : Math.min(min, metrics.latency),
    Number.MAX_SAFE_INTEGER
  );
  const duration = endingTimestamp - startingTimestamp;

  // Calculate the variance
  const variance =
    metrics.reduce((acc, metric) => {
      const diff = metric.latency - averageLatency;
      return acc + diff * diff;
    }, 0) / metrics.length;

  // Calculate the standard deviation
  const standardDeviation = Math.sqrt(variance);

  console.log(
    `\nNumber of Requests: ${chalk.green(
      metrics.length
    )}\nDuration (seconds): ${chalk.green(
      duration
    )}\nRequests per Second: ${chalk.green(
      metrics.length / duration
    )}\nAverage Latency (seconds): ${chalk.green(
      averageLatency
    )}\nAverage Latency (slots): ${chalk.green(
      averageSlots
    )}\nMinimum Latency (seconds) ${chalk.green(
      minLatency
    )}\nMaximum Latency (seconds) ${chalk.green(
      maxLatency
    )}\nStandard Deviation ${chalk.green(standardDeviation)}\n`
  );

  const fileString = `market,openTimestamp,closeTimestamp,latency,price\n${metrics
    .map(
      (m) =>
        `"${m.market}","${m.openTimestamp}","${m.closeTimestamp}","${m.latency}","${m.oraclePrice}"`
    )
    .join("\n")}`;

  const fileName = `${program.programId}.metrics.csv`;
  fs.writeFileSync(fileName, fileString, { encoding: "utf-8" });

  console.log(`Metrics saved to './${fileName}'`);
})();

interface OrderAccount {
  openOrder: number;
  reserved: number[];
  authority: anchor.web3.PublicKey;
  market: anchor.web3.PublicKey;
  marketName: number[];
  openTimestamp: anchor.BN;
  openSlot: anchor.BN;
  closeTimestamp: anchor.BN;
  closeSlot: anchor.BN;
  oraclePrice: anchor.BN;
}
