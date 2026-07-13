/**
 * Devnet setup for the SLAM staking program:
 *   1. initialize  (config + stake vault)
 *   2. init_rewards (reward vault)
 *   3. fund the reward vault with SLAM (devnet: minted fresh; mainnet: from the
 *      30B staking pool custody wallet)
 *
 * Usage: npx ts-node scripts/setup-staking.ts
 */
import * as anchor from '@coral-xyz/anchor';
import { Connection, Keypair, PublicKey, clusterApiUrl } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, mintTo } from '@solana/spl-token';
import * as fs from 'fs';
import * as path from 'path';
import idl from '../target/idl/slam_staking.json';

const REWARD_FUNDING = 10_000_000; // 10M SLAM into the reward vault for devnet testing

async function main() {
  const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');
  const secret = JSON.parse(fs.readFileSync(path.join(__dirname, '../../../devnet-wallet.json'), 'utf-8'));
  const admin = Keypair.fromSecretKey(new Uint8Array(secret));
  const { mint } = JSON.parse(fs.readFileSync(path.join(__dirname, '../.slam-mint-devnet.json'), 'utf-8'));
  const slamMint = new PublicKey(mint);

  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: 'confirmed' });
  const program = new anchor.Program(idl as anchor.Idl, provider);

  const seed = (s: string) => Buffer.from(s);
  const [config] = PublicKey.findProgramAddressSync([seed('staking-config')], program.programId);
  const [vaultAuthority] = PublicKey.findProgramAddressSync([seed('staking-vault-authority')], program.programId);
  const [stakeVault] = PublicKey.findProgramAddressSync([seed('staking-stake-vault')], program.programId);
  const [rewardVault] = PublicKey.findProgramAddressSync([seed('staking-reward-vault')], program.programId);

  console.log('Config:', config.toBase58());
  console.log('Stake vault:', stakeVault.toBase58());
  console.log('Reward vault:', rewardVault.toBase58());

  const existing = await connection.getAccountInfo(config);
  if (!existing) {
    await program.methods.initialize()
      .accounts({
        admin: admin.publicKey, config, vaultAuthority, slamMint, stakeVault,
        tokenProgram: TOKEN_PROGRAM_ID, systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      }).rpc();
    console.log('initialized config + stake vault');
    await program.methods.initRewards()
      .accounts({
        admin: admin.publicKey, config, vaultAuthority, slamMint, rewardVault,
        tokenProgram: TOKEN_PROGRAM_ID, systemProgram: anchor.web3.SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      }).rpc();
    console.log('initialized reward vault');
  } else {
    console.log('config already exists — skipping init');
  }

  // Fund the reward vault (devnet: mint fresh SLAM; admin is the mint authority).
  await mintTo(connection, admin, slamMint, rewardVault, admin.publicKey, REWARD_FUNDING * 1_000_000);
  const bal = await connection.getTokenAccountBalance(rewardVault);
  console.log(`Reward vault funded — balance: ${bal.value.uiAmount} SLAM`);
}

main().catch((e) => { console.error(e); process.exit(1); });
