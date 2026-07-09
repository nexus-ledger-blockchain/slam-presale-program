// Attach Metaplex token metadata (name/symbol/logo URI) to the SLAM mint.
// Signer must be the mint authority (devnet-wallet.json).
//
//   node scripts/attach-token-metadata.mjs
//
// Idempotent-ish: fails with "already in use" if the metadata account exists;
// use updateMetadataAccountV2 for edits (URI stays mutable).
import fs from 'node:fs';
import { createUmi } from '@metaplex-foundation/umi-bundle-defaults';
import { keypairIdentity, publicKey, some, none } from '@metaplex-foundation/umi';
import {
  mplTokenMetadata,
  createMetadataAccountV3,
  findMetadataPda,
  fetchMetadata,
} from '@metaplex-foundation/mpl-token-metadata';

const MINT = publicKey('8KmGd7euYsg3fBbCcc4LnVQhXzkGxAF2t9ZYdUy9BQqC');
const KEYPAIR_PATH = new URL('../../../devnet-wallet.json', import.meta.url).pathname;
const URI = 'https://raw.githubusercontent.com/nexus-ledger-blockchain/slam-presale-program/master/assets/slam-token.json';

const umi = createUmi('https://api.devnet.solana.com').use(mplTokenMetadata());
const secret = new Uint8Array(JSON.parse(fs.readFileSync(KEYPAIR_PATH, 'utf8')));
const kp = umi.eddsa.createKeypairFromSecretKey(secret);
umi.use(keypairIdentity(kp));

console.log('Authority:', kp.publicKey.toString());
console.log('Mint:     ', MINT.toString());

const pda = findMetadataPda(umi, { mint: MINT });
console.log('Metadata PDA:', pda[0].toString());

const { signature } = await createMetadataAccountV3(umi, {
  mint: MINT,
  mintAuthority: umi.identity,
  data: {
    name: 'SLAM',
    symbol: 'SLAM',
    uri: URI,
    sellerFeeBasisPoints: 0,
    creators: none(),
    collection: none(),
    uses: none(),
  },
  isMutable: true,
  collectionDetails: none(),
}).sendAndConfirm(umi);

console.log('Created. Sig:', Buffer.from(signature).toString('hex'));

const md = await fetchMetadata(umi, pda);
console.log('On-chain name:', md.name, '| symbol:', md.symbol);
console.log('On-chain uri: ', md.uri);
