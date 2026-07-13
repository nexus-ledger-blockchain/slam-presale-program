/**
 * Devnet smoke test for governance: ensure the admin has a stake (voting
 * weight), then open a real proposal and read it back from chain.
 *
 * Run: ANCHOR_PROVIDER_URL=https://api.devnet.solana.com \
 *      ANCHOR_WALLET=/home/mikep/work/slam/devnet-wallet.json \
 *      npx ts-node scripts/devnet-gov-smoke.ts
 */
import * as anchor from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddress, TOKEN_PROGRAM_ID } from "@solana/spl-token";

const SLAM_MINT = new PublicKey("8KmGd7euYsg3fBbCcc4LnVQhXzkGxAF2t9ZYdUy9BQqC");
const DEC = 1_000_000;
const STAKE_AMOUNT = 1_000_000 * DEC; // 1,000,000 SLAM — tier 0 (1x) = exactly the proposal threshold

(async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const staking = anchor.workspace.SlamStaking as anchor.Program<any>;
  const gov = anchor.workspace.SlamGovernance as anchor.Program<any>;
  const me = provider.wallet.publicKey;

  const sPda = (s: string) => PublicKey.findProgramAddressSync([Buffer.from(s)], staking.programId)[0];
  const stakingConfig = sPda("staking-config");
  const stakeVault = sPda("staking-stake-vault");
  const stakeAccount = PublicKey.findProgramAddressSync(
    [Buffer.from("stake"), me.toBuffer()], staking.programId)[0];

  const govConfig = PublicKey.findProgramAddressSync([Buffer.from("gov-config")], gov.programId)[0];
  const mySlam = await getAssociatedTokenAddress(SLAM_MINT, me);

  // 1. Stake if we don't already have an active stake.
  const existingStake = await provider.connection.getAccountInfo(stakeAccount);
  if (!existingStake) {
    console.log("staking 1,000,000 SLAM (tier 0, flexible)…");
    const sig = await (staking.methods as any).stake(new anchor.BN(STAKE_AMOUNT), 0)
      .accounts({
        owner: me, config: stakingConfig, stakeAccount, ownerSlamAccount: mySlam,
        stakeVault, tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).rpc();
    console.log("  staked:", sig);
  } else {
    console.log("stake account already exists — reusing it");
  }
  const s = await (staking.account as any).stakeAccount.fetch(stakeAccount);
  console.log("  staked amount:", s.amount.toString(), "| tier:", s.tier);

  // 2. Open a proposal.
  const c = await (gov.account as any).govConfig.fetch(govConfig);
  const id = c.proposalCount.toNumber();
  const proposal = PublicKey.findProgramAddressSync(
    [Buffer.from("gov-proposal"), new anchor.BN(id).toArrayLike(Buffer, "le", 8)], gov.programId)[0];

  console.log(`creating proposal #${id}…`);
  const sig2 = await (gov.methods as any)
    .createProposal(
      "Fund a local reporting grant pool",
      "Set aside treasury SLAM to pay reporters covering under-served local beats. Signaling vote only — the multisig executes.")
    .accounts({
      proposer: me, config: govConfig, proposal, proposerStake: stakeAccount,
      systemProgram: anchor.web3.SystemProgram.programId,
    }).rpc();
  console.log("  created:", sig2);

  const p = await (gov.account as any).proposal.fetch(proposal);
  console.log("\n── proposal on chain ──");
  console.log("  id:      ", p.id.toString());
  console.log("  title:   ", p.title);
  console.log("  status:  ", p.status, "(0=active)");
  console.log("  yes/no:  ", p.yesWeight.toString(), "/", p.noWeight.toString());
  console.log("  ends:    ", new Date(p.votingEnds.toNumber() * 1000).toISOString());
})().catch((e) => { console.error(e); process.exit(1); });
