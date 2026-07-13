/**
 * Verifies set_params against the live devnet governance program: read config,
 * change the voting window, read it back, then restore the intended 5-day value.
 * Also confirms existing proposals still deserialize after the upgrade.
 */
import * as anchor from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";

const FIVE_DAYS = 5 * 24 * 60 * 60;      // 432,000 — the intended production value
const THREE_DAYS = 3 * 24 * 60 * 60;     // 259,200 — a temporary value to prove the write
const MIN_WEIGHT = 1_000_000 * 1_000_000; // 1,000,000 SLAM

(async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const gov = anchor.workspace.SlamGovernance as anchor.Program<any>;
  const [config] = PublicKey.findProgramAddressSync([Buffer.from("gov-config")], gov.programId);
  const read = async () => (gov.account as any).govConfig.fetch(config);

  const before = await read();
  console.log("before      → voting_period:", before.votingPeriodSecs.toString(),
    "| min_weight:", before.minWeightToPropose.toString());

  // Existing state must survive the program upgrade.
  const count = before.proposalCount.toNumber();
  console.log(`existing proposals: ${count}`);
  for (let i = 0; i < count; i++) {
    const [p] = PublicKey.findProgramAddressSync(
      [Buffer.from("gov-proposal"), new anchor.BN(i).toArrayLike(Buffer, "le", 8)], gov.programId);
    const prop = await (gov.account as any).proposal.fetch(p);
    console.log(`  #${prop.id}: "${prop.title}" status=${prop.status} yes=${prop.yesWeight} no=${prop.noWeight}`);
  }

  // Prove the setter writes.
  await (gov.methods as any).setParams(new anchor.BN(THREE_DAYS), new anchor.BN(MIN_WEIGHT))
    .accounts({ admin: provider.wallet.publicKey, config }).rpc();
  const mid = await read();
  console.log("after set 3d→ voting_period:", mid.votingPeriodSecs.toString());
  if (mid.votingPeriodSecs.toNumber() !== THREE_DAYS) throw new Error("set_params did not take effect");

  // Restore the intended production value.
  await (gov.methods as any).setParams(new anchor.BN(FIVE_DAYS), new anchor.BN(MIN_WEIGHT))
    .accounts({ admin: provider.wallet.publicKey, config }).rpc();
  const after = await read();
  console.log("restored    → voting_period:", after.votingPeriodSecs.toString(),
    "| min_weight:", after.minWeightToPropose.toString());
  if (after.votingPeriodSecs.toNumber() !== FIVE_DAYS) throw new Error("restore failed");
  console.log("\nset_params verified on devnet; config back to the 5-day window.");
})().catch((e) => { console.error(e); process.exit(1); });
