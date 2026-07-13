import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  getAccount,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";
import { SlamPresale } from "../target/types/slam_presale";

// Minimal transparent raise: flat $0.00030/token, $1.5M hard cap, $200k soft
// cap, $25k per-wallet max, USDC-only, escrowed + refundable.
//
// This suite drives the FAILED-raise lifecycle end to end: it raises below the
// soft cap and verifies that finalize is rejected, claiming is blocked, and
// every buyer can refund their full contribution from escrow. The escrow
// signed-transfer exercised by refund is the same vault_authority-signed SPL
// transfer that `finalize` uses to sweep to the vault on a successful raise.
const PRICE_MICRO = 300; // $0.00030 per whole SLAM token
const DEC = 1_000_000;

describe("slam_presale — minimal transparent raise", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SlamPresale as Program<SlamPresale>;
  const connection = provider.connection;
  const admin = provider.wallet as anchor.Wallet;

  const [globalState] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("presale-global")],
    program.programId
  );
  const [vaultAuthority] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("presale-vault-authority")],
    program.programId
  );

  let slamMint: anchor.web3.PublicKey;
  let stableMint: anchor.web3.PublicKey;
  let vault: anchor.web3.Keypair;
  let buyer: anchor.web3.Keypair;
  let buyerStable: anchor.web3.PublicKey;
  let escrowStable: anchor.web3.PublicKey;
  const SALE_LEN_S = 8;

  const userPda = (b: anchor.web3.PublicKey) =>
    anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("presale-user"), b.toBuffer()],
      program.programId
    )[0];

  before(async () => {
    vault = anchor.web3.Keypair.generate();
    buyer = anchor.web3.Keypair.generate();
    await connection.confirmTransaction(
      await connection.requestAirdrop(buyer.publicKey, anchor.web3.LAMPORTS_PER_SOL)
    );
    slamMint = await createMint(connection, admin.payer, admin.publicKey, null, 6);
    stableMint = await createMint(connection, admin.payer, admin.publicKey, null, 6);

    const ata = await getOrCreateAssociatedTokenAccount(
      connection, admin.payer, stableMint, buyer.publicKey
    );
    buyerStable = ata.address;
    await mintTo(connection, admin.payer, stableMint, buyerStable, admin.publicKey, 500_000_000); // $500

    escrowStable = anchor.utils.token.associatedAddress({ mint: stableMint, owner: vaultAuthority });
  });

  it("initializes with a short sale window and USDC accepted", async () => {
    const tokenVault = anchor.utils.token.associatedAddress({ mint: slamMint, owner: vaultAuthority });
    const priceFeed = anchor.web3.Keypair.generate();
    const now = Math.floor(Date.now() / 1000);

    await program.methods
      .initialize(new anchor.BN(now - 2), new anchor.BN(now + SALE_LEN_S), [stableMint])
      .accounts({
        admin: admin.publicKey, presaleState: globalState, vaultAuthority,
        slamMint, tokenVault, vault: vault.publicKey, solUsdPriceFeed: priceFeed.publicKey,
      })
      .rpc();

    const s = await program.account.presaleState.fetch(globalState);
    assert.equal(s.acceptedStablesLen, 1);
    assert.isFalse(s.isFinalized);
    assert.isFalse(s.isPaused);
  });

  it("buys at the flat price and escrows the funds", async () => {
    const amount = new anchor.BN(100_000_000); // $100
    await program.methods
      .buyWithStable(amount)
      .accounts({
        buyer: buyer.publicKey, presaleState: globalState, userAllocation: userPda(buyer.publicKey),
        vaultAuthority, stableMint, buyerStableAccount: buyerStable, escrowStableAccount: escrowStable,
        tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    // $100 / $0.00030 = 333,333.333... whole tokens => 333,333,333,333 raw (floor)
    const expected = Math.floor((100_000_000 * DEC) / PRICE_MICRO);
    const alloc = await program.account.userAllocation.fetch(userPda(buyer.publicKey));
    assert.equal(alloc.totalPurchased.toString(), expected.toString());
    assert.equal(alloc.paidStableMicro.toNumber(), 100_000_000);

    // Funds are in ESCROW (vault_authority ATA), not the multisig vault.
    const esc = await getAccount(connection, escrowStable);
    assert.equal(esc.amount.toString(), "100000000");
  });

  it("rejects a contribution above the per-wallet maximum", async () => {
    let failed = false;
    try {
      await program.methods
        .buyWithStable(new anchor.BN(25_000_000_001)) // $25,000.000001 > $25k cap
        .accounts({
          buyer: buyer.publicKey, presaleState: globalState, userAllocation: userPda(buyer.publicKey),
          vaultAuthority, stableMint, buyerStableAccount: buyerStable, escrowStableAccount: escrowStable,
          tokenProgram: TOKEN_PROGRAM_ID, associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([buyer])
        .rpc();
    } catch (e: any) {
      failed = true;
      assert.include(e.toString(), "AboveWalletMaximum");
    }
    assert.isTrue(failed, "expected per-wallet maximum to reject the buy");
  });

  it("waits for the sale window to close", async () => {
    await new Promise((r) => setTimeout(r, (SALE_LEN_S + 2) * 1000));
  }).timeout((SALE_LEN_S + 5) * 1000);

  it("rejects finalize below the soft cap", async () => {
    const vaultStable = (await getOrCreateAssociatedTokenAccount(
      connection, admin.payer, stableMint, vault.publicKey
    )).address;
    let failed = false;
    try {
      await program.methods
        .finalize()
        .accounts({
          admin: admin.publicKey, presaleState: globalState, vaultAuthority, stableMint,
          escrowStableAccount: escrowStable, vaultStableAccount: vaultStable, tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
    } catch (e: any) {
      failed = true;
      assert.include(e.toString(), "SoftCapNotReached");
    }
    assert.isTrue(failed, "expected finalize to reject below soft cap");
  });

  it("blocks enabling claims on an unfinalized raise", async () => {
    const tokenVault = anchor.utils.token.associatedAddress({ mint: slamMint, owner: vaultAuthority });
    let failed = false;
    try {
      await program.methods
        .enableClaim(new anchor.BN(Math.floor(Date.now() / 1000)))
        .accounts({ admin: admin.publicKey, presaleState: globalState, tokenVault })
        .rpc();
    } catch (e: any) {
      failed = true;
      assert.include(e.toString(), "NotFinalized");
    }
    assert.isTrue(failed, "expected enable_claim to require finalization");
  });

  it("refunds the buyer's full contribution from escrow", async () => {
    const before = Number((await getAccount(connection, buyerStable)).amount);

    await program.methods
      .refund()
      .accounts({
        buyer: buyer.publicKey, presaleState: globalState, userAllocation: userPda(buyer.publicKey),
        vaultAuthority, stableMint, escrowStableAccount: escrowStable, buyerStableAccount: buyerStable,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([buyer])
      .rpc();

    const after = Number((await getAccount(connection, buyerStable)).amount);
    assert.equal(after - before, 100_000_000, "buyer should get their full $100 back");

    const esc = await getAccount(connection, escrowStable);
    assert.equal(esc.amount.toString(), "0", "escrow should be drained");

    const alloc = await program.account.userAllocation.fetch(userPda(buyer.publicKey));
    assert.equal(alloc.paidStableMicro.toNumber(), 0);
    assert.equal(alloc.totalPurchased.toNumber(), 0);
  });

  it("rejects a second refund (nothing left)", async () => {
    let failed = false;
    try {
      await program.methods
        .refund()
        .accounts({
          buyer: buyer.publicKey, presaleState: globalState, userAllocation: userPda(buyer.publicKey),
          vaultAuthority, stableMint, escrowStableAccount: escrowStable, buyerStableAccount: buyerStable,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([buyer])
        .rpc();
    } catch (e: any) {
      failed = true;
      assert.include(e.toString(), "NothingToRefund");
    }
    assert.isTrue(failed, "expected second refund to fail");
  });
});
