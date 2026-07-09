/**
 * Runs the one-time `initialize` instruction against devnet, using the mint
 * created by create-mint.ts. Run that first.
 *
 * Usage: npx ts-node scripts/initialize-presale.ts
 */
import * as anchor from '@coral-xyz/anchor';
import { Connection, Keypair, PublicKey, clusterApiUrl } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from '@solana/spl-token';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import idl from '../target/idl/slam_presale.json';

// Confirmed presale window.
const SALE_START = new Date('2026-08-01T00:00:00Z');
const SALE_END = new Date('2027-01-20T23:59:59Z');

// Official Circle devnet USDC mint (see slamweb/src/lib/presale/constants.ts
// for the same value + how it was verified).
const DEVNET_USDC_MINT = new PublicKey('4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU');

// Placeholder — buy_with_sol is not functional yet pending the Pyth
// pull-oracle migration (see that task). Any valid pubkey works structurally
// here; this one is a real devnet Pyth SOL/USD PriceUpdateV2 account found
// during research, kept only so the field isn't garbage, NOT because it's
// confirmed compatible with this program's (outdated) push-oracle code.
const PLACEHOLDER_SOL_USD_FEED = new PublicKey('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');

const GLOBAL_SEED = Buffer.from('presale-global');
const VAULT_AUTHORITY_SEED = Buffer.from('presale-vault-authority');

async function main() {
  const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');
  // Must be the program upgrade authority / SLAM mint authority wallet
  // (Cz3b53Xr...), not ~/.config/solana/id.json which holds an unrelated key.
  const keypairPath = path.join(__dirname, '../../../devnet-wallet.json');
  const secret = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  const admin = Keypair.fromSecretKey(new Uint8Array(secret));

  const mintInfoPath = path.join(__dirname, '../.slam-mint-devnet.json');
  if (!fs.existsSync(mintInfoPath)) {
    throw new Error('Run create-mint.ts first — .slam-mint-devnet.json not found.');
  }
  const { mint: mintAddress } = JSON.parse(fs.readFileSync(mintInfoPath, 'utf-8'));
  const slamMint = new PublicKey(mintAddress);

  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: 'confirmed' });
  const program = new anchor.Program(idl as anchor.Idl, provider);

  const [presaleState] = PublicKey.findProgramAddressSync([GLOBAL_SEED], program.programId);
  const [vaultAuthority] = PublicKey.findProgramAddressSync([VAULT_AUTHORITY_SEED], program.programId);
  const tokenVault = getAssociatedTokenAddressSync(slamMint, vaultAuthority, true);

  // Proceeds vault — using the admin wallet for this initial devnet test.
  // MUST be a real multisig before any deployment that touches real funds.
  const vault = admin.publicKey;

  console.log('Admin:', admin.publicKey.toBase58());
  console.log('Presale state PDA:', presaleState.toBase58());
  console.log('Vault authority PDA:', vaultAuthority.toBase58());
  console.log('SLAM mint:', slamMint.toBase58());
  console.log('Token vault ATA:', tokenVault.toBase58());

  const sig = await program.methods
    .initialize(
      new anchor.BN(Math.floor(SALE_START.getTime() / 1000)),
      new anchor.BN(Math.floor(SALE_END.getTime() / 1000)),
      [DEVNET_USDC_MINT]
    )
    .accounts({
      admin: admin.publicKey,
      presaleState,
      vaultAuthority,
      slamMint,
      tokenVault,
      vault,
      solUsdPriceFeed: PLACEHOLDER_SOL_USD_FEED,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

  console.log('Initialized. Tx:', sig);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
