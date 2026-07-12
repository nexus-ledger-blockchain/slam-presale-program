// Create the tokenomics vesting contracts as Streamflow streams (devnet
// rehearsal — at mainnet, re-run against the mainnet mint with REAL
// recipient/multisig addresses).
//
//   node scripts/create-vesting-streams.mjs
//
// Sender = master authority (devnet-wallet.json). Placeholder recipient
// keypairs are generated in the repo root on first run (gitignored via
// *-wallet.json). Stream IDs are written to vesting-streams.devnet.json.
//
// Schedules (from the published tokenomics):
//   Team & Advisors   22.5B — 24-mo linear, 6-mo cliff (25% at cliff)
//   Liquidity / DEX   15B   — 6-mo linear, no cliff
//   Staking Pool      30B   — 48-mo linear emission to the staking distributor
import fs from 'node:fs';
import { Keypair, PublicKey } from '@solana/web3.js';
import { GenericStreamClient, IChain, ICluster, getBN } from '@streamflow/stream';

const SLAM_MINT = '8KmGd7euYsg3fBbCcc4LnVQhXzkGxAF2t9ZYdUy9BQqC';
const DECIMALS = 6;
const ROOT = new URL('../../../', import.meta.url).pathname;

const loadOrCreate = (name) => {
  const path = ROOT + name;
  if (!fs.existsSync(path)) {
    const kp = Keypair.generate();
    fs.writeFileSync(path, JSON.stringify([...kp.secretKey]), { mode: 0o600 });
    console.log(`created ${name}: ${kp.publicKey.toBase58()}`);
    return kp;
  }
  return Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(path, 'utf8'))));
};

const authority = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(ROOT + 'devnet-wallet.json', 'utf8'))));
const team = loadOrCreate('team-vesting-wallet.json');
const liquidity = loadOrCreate('liquidity-vesting-wallet.json');
const stakingDist = loadOrCreate('staking-distributor-wallet.json');

const client = new GenericStreamClient({
  chain: IChain.Solana,
  clusterUrl: 'https://api.devnet.solana.com',
  cluster: ICluster.Devnet,
});

const DAY = 86_400;
const MONTH = 30 * DAY; // calendar-ish; exact dates set for real at mainnet TGE
const now = Math.floor(Date.now() / 1000);
const start = now + 300; // streams must start in the future

const B = (n) => getBN(n, DECIMALS);

const STREAMS = [
  {
    name: 'SLAM Team & Advisors vesting',
    recipient: team.publicKey.toBase58(),
    amount: B(22_500_000_000),
    cliff: start + 6 * MONTH,
    cliffAmount: B(22_500_000_000 / 4), // 6/24 vested at cliff
    period: DAY,
    // remaining 75% over the following 18 months, daily
    amountPerPeriod: B(Math.floor((22_500_000_000 * 0.75) / (18 * 30))),
    canceleableBySender: true, // team departures — standard practice
  },
  {
    name: 'SLAM Liquidity/DEX unlock',
    recipient: liquidity.publicKey.toBase58(),
    amount: B(15_000_000_000),
    cliff: start,
    cliffAmount: B(0),
    period: DAY,
    amountPerPeriod: B(Math.floor(15_000_000_000 / (6 * 30))),
    canceleableBySender: false,
  },
  {
    name: 'SLAM Staking Pool emission',
    recipient: stakingDist.publicKey.toBase58(),
    amount: B(30_000_000_000),
    cliff: start,
    cliffAmount: B(0),
    period: DAY,
    amountPerPeriod: B(Math.floor(30_000_000_000 / (48 * 30))),
    canceleableBySender: false,
  },
];

const results = [];
for (const s of STREAMS) {
  console.log(`\ncreating: ${s.name} → ${s.recipient}`);
  const { txId, metadataId } = await client.create({
    recipient: s.recipient,
    tokenId: SLAM_MINT,
    start,
    amount: s.amount,
    period: s.period,
    cliff: s.cliff,
    cliffAmount: s.cliffAmount,
    amountPerPeriod: s.amountPerPeriod,
    name: s.name,
    canTopup: false, // false = vesting contract semantics
    cancelableBySender: s.canceleableBySender,
    cancelableByRecipient: false,
    transferableBySender: false,
    transferableByRecipient: false,
    automaticWithdrawal: false,
    partner: authority.publicKey.toBase58(),
  }, { sender: authority });
  console.log(`  stream: ${metadataId}  tx: ${txId}`);
  results.push({ name: s.name, recipient: s.recipient, stream: metadataId, tx: txId });
}

fs.writeFileSync(new URL('../vesting-streams.devnet.json', import.meta.url),
  JSON.stringify({ createdAt: new Date().toISOString(), start, streams: results }, null, 2));
console.log('\nwrote vesting-streams.devnet.json');
