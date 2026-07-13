use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("Fnv8zccRsC7r4FCbjj9pHxHQkd1ZQQA7odHaqDxV7Lau");

#[program]
pub mod slam_presale {
    use super::*;

    /// One-time setup. Bakes in the vault, SOL/USD price feed, sale window,
    /// and accepted stable-coin mints. Round prices/allocations are NOT a
    /// parameter here — they're compile-time constants in `constants.rs` so
    /// they can never be changed post-deployment by whoever holds the admin
    /// key.
    pub fn initialize(
        ctx: Context<Initialize>,
        sale_start_ts: i64,
        sale_end_ts: i64,
        accepted_stables: Vec<Pubkey>,
    ) -> Result<()> {
        initialize::handler(ctx, sale_start_ts, sale_end_ts, accepted_stables)
    }

    /// Buy SLAM with native SOL. `max_sol_lamports` is the most the buyer is
    /// willing to spend; if the current round has less capacity left than
    /// that would buy, only the actual remaining capacity is charged.
    pub fn buy_with_sol(ctx: Context<BuyWithSol>, max_sol_lamports: u64) -> Result<()> {
        buy_with_sol::handler(ctx, max_sol_lamports)
    }

    /// Buy SLAM with an accepted SPL stable-coin (USDC/USDT). `stable_amount`
    /// is in the stable-coin's own base units (6 decimals).
    pub fn buy_with_stable(ctx: Context<BuyWithStable>, stable_amount: u64) -> Result<()> {
        buy_with_stable::handler(ctx, stable_amount)
    }

    /// Admin-only. After the sale window closes, if the soft cap was reached,
    /// sweeps escrowed proceeds to the multisig vault and marks the raise
    /// finalized (the precondition for enabling claims).
    pub fn finalize(ctx: Context<Finalize>) -> Result<()> {
        finalize::handler(ctx)
    }

    /// Buyer-callable. After the sale window closes, if the soft cap was NOT
    /// reached, returns the buyer's full stable contribution from escrow.
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        refund::handler(ctx)
    }

    /// Admin-only. Activates claiming and fixes the TGE timestamp that all
    /// vesting math is computed against. Requires the token vault to already
    /// hold at least `total_tokens_sold` SLAM.
    pub fn enable_claim(ctx: Context<EnableClaim>, tge_timestamp: i64) -> Result<()> {
        enable_claim::handler(ctx, tge_timestamp)
    }

    /// Buyer-callable any time after claiming is enabled. Transfers whatever
    /// portion of their allocation has vested (10% at TGE, linear over the
    /// following 6 months) and hasn't already been claimed.
    pub fn claim_tokens(ctx: Context<ClaimTokens>) -> Result<()> {
        claim_tokens::handler(ctx)
    }

    pub fn set_paused(ctx: Context<AdminOnly>, paused: bool) -> Result<()> {
        admin::set_paused(ctx, paused)
    }

    pub fn update_vault(ctx: Context<AdminOnly>, new_vault: Pubkey) -> Result<()> {
        admin::update_vault(ctx, new_vault)
    }

    pub fn update_price_feed(ctx: Context<AdminOnly>, new_feed: Pubkey) -> Result<()> {
        admin::update_price_feed(ctx, new_feed)
    }

    pub fn update_accepted_stables(ctx: Context<AdminOnly>, mints: Vec<Pubkey>) -> Result<()> {
        admin::update_accepted_stables(ctx, mints)
    }

    pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Pubkey) -> Result<()> {
        admin::transfer_admin(ctx, new_admin)
    }

    /// Admin-only devnet reset: closes the global state (rent back to admin)
    /// so `initialize` can run again. Rejected once any tokens have been
    /// sold. REVIEW BEFORE MAINNET.
    pub fn close_presale_state(ctx: Context<ClosePresaleState>) -> Result<()> {
        admin::close_presale_state(ctx)
    }
}
