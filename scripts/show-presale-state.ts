import * as anchor from '@coral-xyz/anchor';
import { Connection, Keypair, PublicKey, clusterApiUrl } from '@solana/web3.js';
import idl from '../target/idl/slam_presale.json';

async function main() {
  const connection = new Connection(clusterApiUrl('devnet'), 'confirmed');
  const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(Keypair.generate()), {});
  const program = new anchor.Program(idl as anchor.Idl, provider);
  const [statePda] = PublicKey.findProgramAddressSync([Buffer.from('presale-global')], program.programId);
  const s: any = await (program.account as any).presaleState.fetch(statePda);
  console.log('PresaleState:', statePda.toBase58());
  console.log('  admin:', s.admin.toBase58());
  console.log('  saleStart:', new Date(s.saleStartTs.toNumber() * 1000).toISOString());
  console.log('  saleEnd:', new Date(s.saleEndTs.toNumber() * 1000).toISOString());
  console.log('  currentRound:', s.currentRound, ' totalSold:', s.totalTokensSold.toString(), ' raisedMicro:', s.totalUsdRaisedMicro.toString());
  console.log('  paused:', s.isPaused, ' claimActive:', s.isClaimActive);
  console.log('  tokenVault:', s.tokenVault.toBase58());
}
main().catch(e => { console.error(e); process.exit(1); });
