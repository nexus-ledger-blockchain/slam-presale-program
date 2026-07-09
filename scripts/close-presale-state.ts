/**
 * Devnet reset: closes the PresaleState PDA via `close_presale_state` so
 * initialize-presale.ts can be run again (e.g. after recreating the SLAM
 * mint). The program refuses to close once any purchase has been recorded.
 *
 * Usage: npx ts-node scripts/close-presale-state.ts
 */
import * as anchor from '@coral-xyz/anchor';
import { Connection, Keypair, PublicKey, clusterApiUrl } from '@solana/web3.js';
import * as fs from 'fs';
import * as path from 'path';
import idl from '../target/idl/slam_presale.json';

const GLOBAL_SEED = Buffer.from('presale-global');

async function main() {
  const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');
  // Same admin wallet initialize-presale.ts signs with.
  const keypairPath = path.join(__dirname, '../../../devnet-wallet.json');
  const secret = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  const admin = Keypair.fromSecretKey(new Uint8Array(secret));

  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: 'confirmed' });
  const program = new anchor.Program(idl as anchor.Idl, provider);

  const [presaleState] = PublicKey.findProgramAddressSync([GLOBAL_SEED], program.programId);
  const before = await connection.getAccountInfo(presaleState);
  if (!before) {
    console.log('PresaleState', presaleState.toBase58(), 'does not exist — nothing to close.');
    return;
  }

  const sig = await program.methods
    .closePresaleState()
    .accounts({ admin: admin.publicKey, presaleState })
    .rpc();
  console.log('Closed', presaleState.toBase58(), 'Tx:', sig);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
