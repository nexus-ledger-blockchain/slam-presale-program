use crate::constants::*;
use crate::errors::PresaleError;
use crate::math::mul_div_u64;
use anchor_lang::prelude::*;

#[account]
pub struct PresaleState {
    /// Admin authority — can pause, update the vault/price-feed/stable list,
    /// enable claiming, and transfer admin rights. Cannot alter round prices,
    /// round allocations, or a buyer's recorded purchase after the fact.
    pub admin: Pubkey,
    /// Destination for all SOL/stable-coin proceeds (should be a multisig,
    /// not the deploying admin's personal wallet).
    pub vault: Pubkey,
    /// SLAM mint address.
    pub slam_mint: Pubkey,
    /// Program's SLAM token account that claims are paid out of. Must be
    /// funded with at least `total_tokens_sold` SLAM before `enable_claim`.
    pub token_vault: Pubkey,
    /// Pyth price account for SOL/USD. Configurable so a stale/incorrect
    /// address can be fixed without a program upgrade.
    pub sol_usd_price_feed: Pubkey,

    pub accepted_stables: [Pubkey; MAX_ACCEPTED_STABLES],
    pub accepted_stables_len: u8,

    pub sale_start_ts: i64,
    pub sale_end_ts: i64,

    /// 0-indexed current round. Advances automatically as each round sells
    /// out; NUM_ROUNDS once every round is exhausted.
    pub current_round: u8,
    pub round_tokens_sold: [u64; NUM_ROUNDS as usize],

    pub total_tokens_sold: u64,
    /// Cumulative USD raised across all payment methods, in micro-USD.
    pub total_usd_raised_micro: u64,

    pub is_paused: bool,

    pub is_claim_active: bool,
    /// Set once, the first time `enable_claim` runs. Vesting math is relative
    /// to this timestamp.
    pub tge_timestamp: i64,

    pub bump: u8,
    pub vault_authority_bump: u8,
}

impl PresaleState {
    pub const SPACE: usize = 8 // discriminator
        + 32 * 5 // admin, vault, slam_mint, token_vault, sol_usd_price_feed
        + 32 * MAX_ACCEPTED_STABLES
        + 1 // accepted_stables_len
        + 8 * 2 // sale_start_ts, sale_end_ts
        + 1 // current_round
        + 8 * NUM_ROUNDS as usize // round_tokens_sold
        + 8 * 2 // total_tokens_sold, total_usd_raised_micro
        + 1 // is_paused
        + 1 // is_claim_active
        + 8 // tge_timestamp
        + 1 // bump
        + 1; // vault_authority_bump

    pub fn round_price(&self, round: u8) -> u64 {
        ROUND_PRICE_MICRO_USD[round as usize]
    }

    pub fn assert_purchasable(&self, now: i64) -> Result<()> {
        require!(!self.is_paused, PresaleError::Paused);
        require!(now >= self.sale_start_ts, PresaleError::NotStarted);
        require!(now <= self.sale_end_ts, PresaleError::Ended);
        require!(self.current_round < NUM_ROUNDS, PresaleError::SoldOut);
        Ok(())
    }

    /// Consumes a purchase worth `usd_value_micro` (micro-USD) against the
    /// current round, clamping to that round's remaining capacity and
    /// advancing to the next round on an exact fill. Deliberately does NOT
    /// spill over into the next round within a single call — a buyer whose
    /// order exceeds what's left in the current round only fills the current
    /// round's remainder (at the current round's price) and must submit a
    /// second transaction to buy from the next round at its own price. This
    /// avoids blended-price bookkeeping and matches the reference contract's
    /// behavior.
    ///
    /// Returns `(tokens_granted, usd_value_micro_actually_charged)` — the
    /// caller must only transfer/charge the returned USD amount, not the
    /// original `usd_value_micro`, since it may have been clamped down.
    pub fn consume_purchase(&mut self, usd_value_micro: u64, now: i64) -> Result<(u64, u64)> {
        self.assert_purchasable(now)?;
        require!(usd_value_micro > 0, PresaleError::ZeroAmount);
        require!(
            usd_value_micro >= MIN_PURCHASE_MICRO_USD,
            PresaleError::BelowMinimum
        );

        let round = self.current_round;
        let price = self.round_price(round);
        let desired_tokens = mul_div_u64(usd_value_micro, SLAM_DECIMALS_MULTIPLIER, price)?;

        let remaining = ROUND_ALLOCATION_TOKENS
            .checked_sub(self.round_tokens_sold[round as usize])
            .ok_or(PresaleError::MathUnderflow)?;

        let (tokens, actual_usd_micro) = if desired_tokens >= remaining {
            let actual_usd_micro = mul_div_u64(remaining, price, SLAM_DECIMALS_MULTIPLIER)?;
            (remaining, actual_usd_micro)
        } else {
            (desired_tokens, usd_value_micro)
        };

        self.round_tokens_sold[round as usize] = self.round_tokens_sold[round as usize]
            .checked_add(tokens)
            .ok_or(PresaleError::MathOverflow)?;
        self.total_tokens_sold = self
            .total_tokens_sold
            .checked_add(tokens)
            .ok_or(PresaleError::MathOverflow)?;
        self.total_usd_raised_micro = self
            .total_usd_raised_micro
            .checked_add(actual_usd_micro)
            .ok_or(PresaleError::MathOverflow)?;

        if tokens == remaining {
            self.current_round = self
                .current_round
                .checked_add(1)
                .ok_or(PresaleError::MathOverflow)?;
        }

        Ok((tokens, actual_usd_micro))
    }
}

/// Vesting: 10% unlocks at TGE, the remaining 90% unlocks linearly over
/// `VESTING_DURATION_SECONDS`. Matches the published Tokenomics page.
pub fn total_vested_amount(total_purchased: u64, tge_timestamp: i64, now: i64) -> Result<u64> {
    if now <= tge_timestamp {
        return Ok(0);
    }
    let elapsed = now.saturating_sub(tge_timestamp);

    if elapsed >= VESTING_DURATION_SECONDS {
        return Ok(total_purchased);
    }

    let tge_amount = mul_div_u64(total_purchased, TGE_UNLOCK_BPS, BPS_DENOMINATOR)?;
    let remaining_after_tge = total_purchased
        .checked_sub(tge_amount)
        .ok_or(PresaleError::MathUnderflow)?;
    let linear_vested = mul_div_u64(
        remaining_after_tge,
        elapsed as u64,
        VESTING_DURATION_SECONDS as u64,
    )?;

    tge_amount
        .checked_add(linear_vested)
        .ok_or(PresaleError::MathOverflow.into())
}

#[account]
pub struct UserAllocation {
    pub buyer: Pubkey,
    /// Total SLAM purchased across every round and every payment method, in
    /// SLAM base units (6 decimals).
    pub total_purchased: u64,
    /// How much of `total_purchased` has already been transferred out via
    /// `claim_tokens`.
    pub total_claimed: u64,
    pub paid_sol_lamports: u64,
    pub paid_stable_micro: u64,
    pub bump: u8,
}

impl UserAllocation {
    pub const SPACE: usize = 8 // discriminator
        + 32 // buyer
        + 8 * 4 // total_purchased, total_claimed, paid_sol_lamports, paid_stable_micro
        + 1; // bump
}
