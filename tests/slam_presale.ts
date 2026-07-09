import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { assert } from "chai";
import { SlamPresale } from "../target/types/slam_presale";

describe("slam_presale", () => {
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
  let vault: anchor.web3.Keypair;
  let buyer: anchor.web3.Keypair;

  before(async () => {
    vault = anchor.web3.Keypair.generate();
    slamMint = await createMint(
      connection,
      admin.payer,
      admin.publicKey,
      null,
      6 // SLAM_DECIMALS — see constants.rs for why this must stay 6
    );
  });

  it("initializes the presale", async () => {
    const tokenVault = anchor.utils.token.associatedAddress({
      mint: slamMint,
      owner: vaultAuthority,
    });

    // Dummy price feed account for localnet — a real Pyth feed address is
    // required on devnet/mainnet.
    const priceFeed = anchor.web3.Keypair.generate();

    const now = Math.floor(Date.now() / 1000);
    await program.methods
      .initialize(
        new anchor.BN(now - 60),
        new anchor.BN(now + 60 * 60 * 24 * 30),
        []
      )
      .accounts({
        admin: admin.publicKey,
        presaleState: globalState,
        vaultAuthority,
        slamMint,
        tokenVault,
        vault: vault.publicKey,
        solUsdPriceFeed: priceFeed.publicKey,
      })
      .rpc();

    const state = await program.account.presaleState.fetch(globalState);
    assert.equal(state.currentRound, 0);
    assert.equal(state.totalTokensSold.toNumber(), 0);
    assert.isFalse(state.isPaused);
  });

  it("buys with USDC-equivalent stable coin and fills round 1 at the correct price", async () => {
    buyer = anchor.web3.Keypair.generate();
    await connection.confirmTransaction(
      await connection.requestAirdrop(buyer.publicKey, anchor.web3.LAMPORTS_PER_SOL)
    );

    const stableMint = await createMint(
      connection,
      admin.payer,
      admin.publicKey,
      null,
      6
    );

    await program.methods
      .updateAcceptedStables([stableMint])
      .accounts({ admin: admin.publicKey, presaleState: globalState })
      .rpc();

    const buyerStableAccount = await getOrCreateAssociatedTokenAccount(
      connection,
      admin.payer,
      stableMint,
      buyer.publicKey
    );
    await mintTo(
      connection,
      admin.payer,
      stableMint,
      buyerStableAccount.address,
      admin.publicKey,
      1_000_000_000 // 1000 stable-coin units, plenty to test a small buy
    );

    const vaultStableAccount = await getOrCreateAssociatedTokenAccount(
      connection,
      admin.payer,
      stableMint,
      vault.publicKey
    );

    const [userAllocation] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("presale-user"), buyer.publicKey.toBuffer()],
      program.programId
    );

    // $100 at round 1's price (100 micro-USD/token) => exactly 1,000,000 SLAM
    // = 1e12 raw units at 6 decimals.
    const stableAmount = new anchor.BN(100_000_000); // $100.00 in 6-decimal units

    await program.methods
      .buyWithStable(stableAmount)
      .accounts({
        buyer: buyer.publicKey,
        presaleState: globalState,
        userAllocation,
        stableMint,
        buyerStableAccount: buyerStableAccount.address,
        vaultStableAccount: vaultStableAccount.address,
      })
      .signers([buyer])
      .rpc();

    const allocation = await program.account.userAllocation.fetch(userAllocation);
    assert.equal(allocation.totalPurchased.toNumber(), 1_000_000_000_000);
    assert.equal(allocation.paidStableMicro.toNumber(), 100_000_000);

    const state = await program.account.presaleState.fetch(globalState);
    assert.equal(state.totalTokensSold.toNumber(), 1_000_000_000_000);
    assert.equal(state.totalUsdRaisedMicro.toNumber(), 100_000_000);
  });

  it("enables claim and pays out the 10% TGE unlock", async () => {
    const tokenVault = anchor.utils.token.associatedAddress({
      mint: slamMint,
      owner: vaultAuthority,
    });

    const state = await program.account.presaleState.fetch(globalState);
    const totalSold = BigInt(state.totalTokensSold.toString());

    // enable_claim requires the vault to cover everything sold.
    await mintTo(
      connection,
      admin.payer,
      slamMint,
      tokenVault,
      admin.publicKey,
      totalSold
    );

    // TGE a few seconds in the past so the claim window is open; the linear
    // component accrued in those seconds is negligible next to the 10% unlock.
    const tge = Math.floor(Date.now() / 1000) - 10;
    await program.methods
      .enableClaim(new anchor.BN(tge))
      .accounts({ admin: admin.publicKey, presaleState: globalState, tokenVault })
      .rpc();

    const [userAllocation] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("presale-user"), buyer.publicKey.toBuffer()],
      program.programId
    );

    await program.methods
      .claimTokens()
      .accounts({
        buyer: buyer.publicKey,
        presaleState: globalState,
        userAllocation,
        vaultAuthority,
        tokenVault,
        slamMint,
      })
      .signers([buyer])
      .rpc();

    const buyerSlamAccount = anchor.utils.token.associatedAddress({
      mint: slamMint,
      owner: buyer.publicKey,
    });
    const received = BigInt(
      (await connection.getTokenAccountBalance(buyerSlamAccount)).value.amount
    );

    // Exactly 10% TGE unlock plus < 0.01% of linear drift for elapsed seconds.
    const tgeUnlock = totalSold / 10n;
    assert.isTrue(received >= tgeUnlock, `received ${received} < TGE unlock ${tgeUnlock}`);
    assert.isTrue(received < tgeUnlock + totalSold / 10_000n, `received ${received} far above TGE unlock ${tgeUnlock}`);

    const allocation = await program.account.userAllocation.fetch(userAllocation);
    assert.equal(allocation.totalClaimed.toString(), received.toString());
  });
});
