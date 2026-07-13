/**
 * One-time devnet setup for slam_governance: initialize the GovConfig, pointing
 * it at the deployed slam_staking program (the source of voting weight).
 *
 * Run: npx ts-node scripts/setup-governance.ts
 */
import * as anchor from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";

const STAKING_PROGRAM = new PublicKey("FLaXjknGBuX9FYPLzs3CKecYYWVgYuC8nTQequTMxAcH");

// Voting window: 5 days. Proposal threshold: 1,000,000 SLAM of voting weight
// (staked amount x tier multiplier) — enough to deter spam, low enough to use.
const VOTING_PERIOD_SECS = 5 * 24 * 60 * 60;
const MIN_WEIGHT_TO_PROPOSE = 1_000_000 * 1_000_000; // 1,000,000 SLAM at 6 decimals

(async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SlamGovernance as anchor.Program<any>;

  const [config] = PublicKey.findProgramAddressSync([Buffer.from("gov-config")], program.programId);
  console.log("governance program:", program.programId.toBase58());
  console.log("gov config PDA:   ", config.toBase58());

  const existing = await provider.connection.getAccountInfo(config);
  if (existing) {
    const c = await (program.account as any).govConfig.fetch(config);
    console.log("Already initialized. staking_program:", c.stakingProgram.toBase58(),
      "| proposals:", c.proposalCount.toString(),
      "| voting period:", c.votingPeriodSecs.toString(), "s");
    return;
  }

  const sig = await (program.methods as any)
    .initialize(STAKING_PROGRAM, new anchor.BN(VOTING_PERIOD_SECS), new anchor.BN(MIN_WEIGHT_TO_PROPOSE))
    .accounts({
      admin: provider.wallet.publicKey,
      config,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

  console.log("initialized:", sig);
  const c = await (program.account as any).govConfig.fetch(config);
  console.log("staking_program:", c.stakingProgram.toBase58());
  console.log("voting_period_secs:", c.votingPeriodSecs.toString());
  console.log("min_weight_to_propose:", c.minWeightToPropose.toString());
})().catch((e) => { console.error(e); process.exit(1); });
