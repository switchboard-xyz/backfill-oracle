import type { BackfillOracleProgram } from "../target/types/backfill_oracle_program";

import type { Program } from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";

describe("backfill-oracle-program", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .BackfillOracleProgram as Program<BackfillOracleProgram>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
