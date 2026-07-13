import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint, getOrCreateAssociatedTokenAccount, mintTo, getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";
import { SlamStaking } from "../target/types/slam_staking";

const DEC = 1_000_000;

describe("slam_staking — fixed-APY lock tiers", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SlamStaking as Program<SlamStaking>;
  const connection = provider.connection;
  const admin = provider.wallet as anchor.Wallet;

  const pda = (seeds: (Buffer | Uint8Array)[]) =>
    anchor.web3.PublicKey.findProgramAddressSync(seeds, program.programId)[0];

  const config = pda([Buffer.from("staking-config")]);
  const vaultAuthority = pda([Buffer.from("staking-vault-authority")]);
  const stakeVault = pda([Buffer.from("staking-stake-vault")]);
  const rewardVault = pda([Buffer.from("staking-reward-vault")]);
  const stakePda = (owner: anchor.web3.PublicKey) => pda([Buffer.from("stake"), owner.toBuffer()]);

  let slamMint: anchor.web3.PublicKey;
  let user: anchor.web3.Keypair;
  let userSlam: anchor.web3.PublicKey;

  before(async () => {
    slamMint = await createMint(connection, admin.payer, admin.publicKey, null, 6);
    user = anchor.web3.Keypair.generate();
    await connection.confirmTransaction(
      await connection.requestAirdrop(user.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL)
    );
    userSlam = (await getOrCreateAssociatedTokenAccount(connection, admin.payer, slamMint, user.publicKey)).address;
    // Give the user 2,000,000 SLAM to stake with.
    await mintTo(connection, admin.payer, slamMint, userSlam, admin.publicKey, 2_000_000 * DEC);
  });

  it("initializes config + stake vault, then reward vault", async () => {
    await program.methods.initialize()
      .accounts({
        admin: admin.publicKey, config, vaultAuthority, slamMint, stakeVault,
        tokenProgram: TOKEN_PROGRAM_ID, systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      }).rpc();
    await program.methods.initRewards()
      .accounts({
        admin: admin.publicKey, config, vaultAuthority, slamMint, rewardVault,
        tokenProgram: TOKEN_PROGRAM_ID, systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      }).rpc();
    // Fund the reward vault with 500,000 SLAM from the "pool".
    await mintTo(connection, admin.payer, slamMint, rewardVault, admin.publicKey, 500_000 * DEC);

    const c = await program.account.stakingConfig.fetch(config);
    assert.equal(c.totalStaked.toNumber(), 0);
    assert.equal(c.rewardVault.toBase58(), rewardVault.toBase58());
  });

  it("stakes into the Flexible tier", async () => {
    const amount = new anchor.BN(1_000_000 * DEC); // 1,000,000 SLAM
    await program.methods.stake(amount, 0)
      .accounts({
        owner: user.publicKey, config, stakeAccount: stakePda(user.publicKey),
        ownerSlamAccount: userSlam, stakeVault, tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([user]).rpc();

    const sv = await getAccount(connection, stakeVault);
    assert.equal(sv.amount.toString(), (1_000_000 * DEC).toString());
    const s = await program.account.stakeAccount.fetch(stakePda(user.publicKey));
    assert.equal(s.tier, 0);
    assert.equal(s.amount.toString(), (1_000_000 * DEC).toString());
    const c = await program.account.stakingConfig.fetch(config);
    assert.equal(c.totalStaked.toString(), (1_000_000 * DEC).toString());
  });

  it("accrues and claims a reward", async () => {
    await new Promise((r) => setTimeout(r, 4000)); // let a few seconds of APY accrue
    const before = Number((await getAccount(connection, userSlam)).amount);
    await program.methods.claim()
      .accounts({
        owner: user.publicKey, config, stakeAccount: stakePda(user.publicKey),
        vaultAuthority, rewardVault, ownerSlamAccount: userSlam, tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([user]).rpc();
    const after = Number((await getAccount(connection, userSlam)).amount);
    const reward = after - before;
    // 1,000,000 SLAM at 6% APY over ~4s ≈ 7,600 base units. Just assert > 0 and sane.
    assert.isTrue(reward > 0, `expected a positive reward, got ${reward}`);
    assert.isTrue(reward < 1 * DEC, `reward ${reward} unexpectedly large`);
    const s = await program.account.stakeAccount.fetch(stakePda(user.publicKey));
    assert.equal(s.rewardClaimed.toString(), reward.toString());
  }).timeout(10000);

  it("unstakes the Flexible tier — full principal returned, account closed", async () => {
    const before = Number((await getAccount(connection, userSlam)).amount);
    await program.methods.unstake()
      .accounts({
        owner: user.publicKey, config, stakeAccount: stakePda(user.publicKey),
        vaultAuthority, stakeVault, rewardVault, ownerSlamAccount: userSlam,
        tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([user]).rpc();
    const after = Number((await getAccount(connection, userSlam)).amount);
    const returned = after - before;
    // Full 1,000,000 principal + a tiny extra reward, no penalty.
    assert.isTrue(returned >= 1_000_000 * DEC, `expected >= principal, got ${returned}`);
    assert.isTrue(returned < 1_000_001 * DEC, `returned ${returned} too high`);
    const c = await program.account.stakingConfig.fetch(config);
    assert.equal(c.totalStaked.toNumber(), 0);
    // Stake account closed.
    const closed = await connection.getAccountInfo(stakePda(user.publicKey));
    assert.isNull(closed, "stake account should be closed");
  });

  it("applies the early-unstake penalty on a locked tier", async () => {
    // Stake into tier 2 (12-month lock, 15% early penalty), then unstake early.
    const amount = 1_000_000 * DEC;
    await program.methods.stake(new anchor.BN(amount), 2)
      .accounts({
        owner: user.publicKey, config, stakeAccount: stakePda(user.publicKey),
        ownerSlamAccount: userSlam, stakeVault, tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([user]).rpc();

    const rewardVaultBefore = Number((await getAccount(connection, rewardVault)).amount);
    const userBefore = Number((await getAccount(connection, userSlam)).amount);

    await program.methods.unstake()
      .accounts({
        owner: user.publicKey, config, stakeAccount: stakePda(user.publicKey),
        vaultAuthority, stakeVault, rewardVault, ownerSlamAccount: userSlam,
        tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([user]).rpc();

    const userAfter = Number((await getAccount(connection, userSlam)).amount);
    const returned = userAfter - userBefore;
    const penalty = 0.15 * amount; // 150,000 SLAM
    // Returned = 85% principal + tiny reward. Should be very close to 850,000.
    assert.isTrue(returned >= 0.85 * amount, `expected >= 85% back, got ${returned}`);
    assert.isTrue(returned < 0.85 * amount + 1 * DEC, `returned ${returned} above expected`);

    // Penalty recycled into the reward vault (minus the tiny reward just paid out).
    const rewardVaultAfter = Number((await getAccount(connection, rewardVault)).amount);
    const vaultDelta = rewardVaultAfter - rewardVaultBefore;
    assert.isTrue(vaultDelta > 0.14 * amount, `reward vault should grow ~penalty, grew ${vaultDelta}`);
  });

  // ── Governance (reads voting weight from the staking StakeAccount above) ──
  const gov = anchor.workspace.SlamGovernance as anchor.Program<any>;
  const govConfig = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("gov-config")], gov.programId)[0];
  const proposalPda = (id: number) =>
    anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("gov-proposal"), new anchor.BN(id).toArrayLike(Buffer, "le", 8)], gov.programId)[0];
  const votePda = (proposal: anchor.web3.PublicKey, voter: anchor.web3.PublicKey) =>
    anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("gov-vote"), proposal.toBuffer(), voter.toBuffer()], gov.programId)[0];

  it("initializes governance and runs a stake-weighted proposal end to end", async () => {
    // Give `user` an active tier-2 stake (2x voting weight).
    await program.methods.stake(new anchor.BN(1_000_000 * DEC), 2)
      .accounts({
        owner: user.publicKey, config, stakeAccount: stakePda(user.publicKey),
        ownerSlamAccount: userSlam, stakeVault, tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([user]).rpc();

    // Init governance: 3-second voting window, min weight 1 to propose.
    await gov.methods.initialize(program.programId, new anchor.BN(3), new anchor.BN(1))
      .accounts({ admin: admin.publicKey, config: govConfig, systemProgram: anchor.web3.SystemProgram.programId })
      .rpc();

    // user creates a proposal (has stake weight).
    await gov.methods.createProposal("Fund community grants", "Allocate treasury to local reporting grants.")
      .accounts({
        proposer: user.publicKey, config: govConfig, proposal: proposalPda(0),
        proposerStake: stakePda(user.publicKey), systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([user]).rpc();

    // user votes YES; weight = 1,000,000 SLAM * 2.0 (tier 2) = 2,000,000.
    await gov.methods.castVote(1)
      .accounts({
        voter: user.publicKey, config: govConfig, proposal: proposalPda(0),
        voteRecord: votePda(proposalPda(0), user.publicKey),
        voterStake: stakePda(user.publicKey), systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([user]).rpc();

    const mid = await gov.account.proposal.fetch(proposalPda(0));
    assert.equal(mid.yesWeight.toString(), (2_000_000 * DEC).toString(), "tier-2 weight should be 2x");
    assert.equal(mid.noWeight.toNumber(), 0);

    // Double-vote is rejected.
    let doubled = false;
    try {
      await gov.methods.castVote(0)
        .accounts({
          voter: user.publicKey, config: govConfig, proposal: proposalPda(0),
          voteRecord: votePda(proposalPda(0), user.publicKey),
          voterStake: stakePda(user.publicKey), systemProgram: anchor.web3.SystemProgram.programId,
        }).signers([user]).rpc();
    } catch { doubled = true; }
    assert.isTrue(doubled, "second vote should fail");

    // Wait for voting to close, then finalize → PASSED.
    await new Promise((r) => setTimeout(r, 4000));
    await gov.methods.finalize().accounts({ proposal: proposalPda(0) }).rpc();
    const done = await gov.account.proposal.fetch(proposalPda(0));
    assert.equal(done.status, 1, "proposal should be PASSED (status 1)");
  }).timeout(15000);

  it("lets the admin retune params, and rejects a non-admin", async () => {
    await gov.methods.setParams(new anchor.BN(600), new anchor.BN(5_000 * DEC))
      .accounts({ admin: admin.publicKey, config: govConfig })
      .rpc();
    const c = await gov.account.govConfig.fetch(govConfig);
    assert.equal(c.votingPeriodSecs.toNumber(), 600);
    assert.equal(c.minWeightToPropose.toString(), (5_000 * DEC).toString());

    // A stranger cannot change them.
    let denied = false;
    try {
      await gov.methods.setParams(new anchor.BN(60), new anchor.BN(1))
        .accounts({ admin: user.publicKey, config: govConfig })
        .signers([user]).rpc();
    } catch { denied = true; }
    assert.isTrue(denied, "non-admin set_params should fail");

    // A zero-length window would open proposals already closed — rejected.
    let badPeriod = false;
    try {
      await gov.methods.setParams(new anchor.BN(0), new anchor.BN(1))
        .accounts({ admin: admin.publicKey, config: govConfig }).rpc();
    } catch { badPeriod = true; }
    assert.isTrue(badPeriod, "zero voting period should be rejected");
  });

  it("rejects voting with someone else's stake account", async () => {
    // Open a fresh proposal (config now has a 600s window).
    const c = await gov.account.govConfig.fetch(govConfig);
    const id = c.proposalCount.toNumber();
    await gov.methods.createProposal("Second proposal", "Checks stake-ownership binding.")
      .accounts({
        proposer: user.publicKey, config: govConfig, proposal: proposalPda(id),
        proposerStake: stakePda(user.publicKey), systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([user]).rpc();

    // `admin` tries to vote using `user`'s stake account — must fail.
    let rejected = false;
    try {
      await gov.methods.castVote(1)
        .accounts({
          voter: admin.publicKey, config: govConfig, proposal: proposalPda(id),
          voteRecord: votePda(proposalPda(id), admin.publicKey),
          voterStake: stakePda(user.publicKey), // not admin's stake
          systemProgram: anchor.web3.SystemProgram.programId,
        }).rpc();
    } catch { rejected = true; }
    assert.isTrue(rejected, "voting with another wallet's stake must fail");
  });

  it("rejects a stake created AFTER the proposal (anti-recycling snapshot)", async () => {
    // Open a proposal first (proposer = user, whose stake predates it).
    const c = await gov.account.govConfig.fetch(govConfig);
    const id = c.proposalCount.toNumber();
    await gov.methods.createProposal("Snapshot test", "Late stakers must not be able to vote.")
      .accounts({
        proposer: user.publicKey, config: govConfig, proposal: proposalPda(id),
        proposerStake: stakePda(user.publicKey), systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([user]).rpc();

    // A brand-new staker enters AFTER the proposal opened.
    const late = anchor.web3.Keypair.generate();
    await connection.confirmTransaction(
      await connection.requestAirdrop(late.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL));
    const lateSlam = (await getOrCreateAssociatedTokenAccount(connection, admin.payer, slamMint, late.publicKey)).address;
    await mintTo(connection, admin.payer, slamMint, lateSlam, admin.publicKey, 1_000_000 * DEC);
    await program.methods.stake(new anchor.BN(1_000_000 * DEC), 0)
      .accounts({
        owner: late.publicKey, config, stakeAccount: stakePda(late.publicKey),
        ownerSlamAccount: lateSlam, stakeVault, tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      }).signers([late]).rpc();

    // Their vote must be rejected — the stake postdates the proposal.
    let blocked = false;
    try {
      await gov.methods.castVote(1)
        .accounts({
          voter: late.publicKey, config: govConfig, proposal: proposalPda(id),
          voteRecord: votePda(proposalPda(id), late.publicKey),
          voterStake: stakePda(late.publicKey), systemProgram: anchor.web3.SystemProgram.programId,
        }).signers([late]).rpc();
    } catch (e: any) { blocked = /StakeTooNew/.test(e.toString()); }
    assert.isTrue(blocked, "a stake created after the proposal must not be able to vote");
  });
});
