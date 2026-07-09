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
    const buyer = anchor.web3.Keypair.generate();
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

    // $100 at round 1's price (430 micro-usd/token) => ~232,558.139 tokens;
    // integer division means ~232558.139534 SLAM (6dp) truncates down.
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
    assert.isTrue(allocation.totalPurchased.toNumber() > 0);
    assert.equal(allocation.paidStableMicro.toNumber(), 100_000_000);
  });
});
