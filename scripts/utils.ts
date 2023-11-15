import type { BackfillOracleProgram } from "../target/types/backfill_oracle_program";

import * as anchor from "@coral-xyz/anchor";
import fs from "fs";
import os from "os";
import path from "path";

export function loadProgram(): [
  anchor.Program<BackfillOracleProgram>,
  anchor.web3.Keypair
] {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(
    process.argv.length > 2
      ? new anchor.AnchorProvider(
          provider.connection,
          new anchor.Wallet(loadKeypair(process.argv[2])),
          {}
        )
      : provider
  );

  const program: anchor.Program<BackfillOracleProgram> =
    anchor.workspace.BackfillOracleProgram;

  const payer = (provider.wallet as anchor.Wallet).payer;
  console.log(`[env] PAYER: ${payer.publicKey}`);

  return [program, payer];
}

export function loadMarkets(
  program: anchor.Program<BackfillOracleProgram>
): [
  anchor.web3.PublicKey,
  anchor.web3.PublicKey,
  anchor.web3.PublicKey,
  anchor.web3.PublicKey
] {
  const [programPubkey] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("PROGRAM")],
    program.programId
  );
  const [btcMarket] = anchor.web3.PublicKey.findProgramAddressSync(
    [programPubkey.toBytes(), Buffer.from(getMarketNameBytes("BTC"))],
    program.programId
  );
  const [ethMarket] = anchor.web3.PublicKey.findProgramAddressSync(
    [programPubkey.toBytes(), Buffer.from(getMarketNameBytes("ETH"))],
    program.programId
  );
  const [solMarket] = anchor.web3.PublicKey.findProgramAddressSync(
    [programPubkey.toBytes(), Buffer.from(getMarketNameBytes("SOL"))],
    program.programId
  );

  return [programPubkey, btcMarket, ethMarket, solMarket];
}

export function loadKeypair(keypairPath: string): anchor.web3.Keypair {
  const fullPath =
    keypairPath.startsWith("/") || keypairPath.startsWith("C:")
      ? keypairPath
      : keypairPath.startsWith("~")
      ? os.homedir() + keypairPath.slice(1)
      : path.join(process.cwd(), keypairPath);

  if (!fs.existsSync(fullPath)) {
    const keypair = anchor.web3.Keypair.generate();
    const dir = path.dirname(fullPath);
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
    fs.writeFileSync(fullPath, `[${keypair.secretKey}]`);
    return keypair;
  }

  return anchor.web3.Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(fullPath, "utf-8")))
  );
}

export function getMarketNameBytes(name: String): number[] {
  if (name.length > 8) {
    throw new Error("String length exceeds 8 characters.");
  }

  const array = new Uint8Array(8);
  for (let i = 0; i < name.length; i++) {
    array[i] = name.charCodeAt(i);
  }

  return Array.from(array);
}
