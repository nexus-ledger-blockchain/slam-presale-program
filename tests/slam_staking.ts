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
});
