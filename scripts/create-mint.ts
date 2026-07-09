/**
 * Creates the SLAM mint on devnet.
 *
 * Classic SPL Token, NOT Token-2022: the presale program uses
 * `anchor_spl::token::Token` in every instruction, so a Token-2022 mint
 * fails account validation at initialize. The planned Token-2022 extensions
 * (transfer-fee etc. — see task "Add Token-2022 extensions to the SLAM
 * mint") require migrating the program to `token_interface` first; when that
 * lands, this mint must be recreated and the presale re-initialized
 * (scripts/close-presale-state.ts exists for exactly that reset).
 *
 * Usage: npx ts-node scripts/create-mint.ts
 */
import { Connection, Keypair, clusterApiUrl } from '@solana/web3.js';
import { createMint, TOKEN_PROGRAM_ID } from '@solana/spl-token';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

const SLAM_DECIMALS = 6; // see constants.rs for why this can't be 9

async function main() {
  const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');
  // Same authority wallet the presale admin uses (Cz3b53Xr...), not
  // ~/.config/solana/id.json which holds an unrelated key.
  const keypairPath = path.join(__dirname, '../../../devnet-wallet.json');
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
    TOKEN_PROGRAM_ID
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
