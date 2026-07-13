import * as anchor from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
(async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const gov = anchor.workspace.SlamGovernance as anchor.Program<any>;
  const [cfg] = PublicKey.findProgramAddressSync([Buffer.from("gov-config")], gov.programId);
  const c = await (gov.account as any).govConfig.fetch(cfg);
  const n = c.proposalCount.toNumber();
  for (let i = 0; i < n; i++) {
    const [p] = PublicKey.findProgramAddressSync(
      [Buffer.from("gov-proposal"), new anchor.BN(i).toArrayLike(Buffer, "le", 8)], gov.programId);
    const pr = await (gov.account as any).proposal.fetch(p);
    console.log(`#${pr.id}: "${pr.title}" | yes=${pr.yesWeight.toNumber() / 1e6} no=${pr.noWeight.toNumber() / 1e6} SLAM`);
  }
})().catch((e) => { console.error(String(e)); process.exit(1); });
