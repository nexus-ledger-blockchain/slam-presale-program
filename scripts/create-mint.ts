/**
 * Creates the SLAM SPL Token-2022 mint on devnet.
 *
 * Deliberately a PLAIN Token-2022 mint for now (no transfer-fee/permanent-
 * delegate/interest-bearing extensions yet) to get the presale mechanism
 * proven end-to-end first — see task "Add Token-2022 extensions to the SLAM
 * mint" for that follow-up. Extensions can only be set at mint-creation
 * time, so this mint will need to be recreated (and the presale
 * re-initialized) once those are added.
 *
 * Usage: npx ts-node scripts/create-mint.ts
 */
import { Connection, Keypair, clusterApiUrl } from '@solana/web3.js';
import { createMint, TOKEN_2022_PROGRAM_ID } from '@solana/spl-token';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

const SLAM_DECIMALS = 6; // see constants.rs for why this can't be 9

async function main() {
  const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');
  const keypairPath = path.join(os.homedir(), '.config/solana/id.json');
  const secret = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  const payer = Keypair.fromSecretKey(new Uint8Array(secret));

  console.log('Payer:', payer.publicKey.toBase58());
  const balance = await connection.getBalance(payer.publicKey);
  console.log('Balance:', balance / 1e9, 'SOL');
  if (balance < 0.05 * 1e9) {
    throw new Error('Insufficient devnet SOL to create the mint — fund the payer first.');
  }

  const mint = await createMint(
    connection,
    payer,
    payer.publicKey, // mint authority — burned later per tokenomics, once the full 150B supply is minted to the allocation wallets
    null, // no freeze authority
    SLAM_DECIMALS,
    undefined,
    undefined,
    TOKEN_2022_PROGRAM_ID
  );

  console.log('SLAM mint created:', mint.toBase58());
  fs.writeFileSync(
    path.join(__dirname, '../.slam-mint-devnet.json'),
    JSON.stringify({ mint: mint.toBase58(), decimals: SLAM_DECIMALS, cluster: 'devnet' }, null, 2)
  );
  console.log('Saved to .slam-mint-devnet.json');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
