use crate::constants::*;
use crate::errors::StakingError;
use anchor_lang::prelude::*;

#[account]
pub struct StakingConfig {
    pub admin: Pubkey,
    pub slam_mint: Pubkey,
    /// Program-owned ATA (of the vault-authority PDA) holding all staked
    /// principal.
    pub stake_vault: Pubkey,
    /// Program-owned ATA holding reward tokens, funded by the admin from the
    /// pre-allocated staking pool.
    pub reward_vault: Pubkey,
    pub total_staked: u64,
    pub is_paused: bool,
    pub bump: u8,
    pub vault_authority_bump: u8,
}

impl StakingConfig {
    pub const SPACE: usize = 8 + 32 * 4 + 8 + 1 + 1 + 1;
}

#[account]
pub struct StakeAccount {
    pub owner: Pubkey,
    /// Principal currently staked, in base units.
    pub amount: u64,
    /// 0 = Flexible, 1 = 6-month, 2 = 12-month.
    pub tier: u8,
    pub staked_at: i64,
    /// staked_at + tier lock; 0 for Flexible.
    pub lock_end: i64,
    /// Timestamp rewards were last accrued to (claim/stake). Reward math is
    /// relative to this.
    pub last_claim: i64,
    /// Cumulative rewards claimed, for display.
    pub reward_claimed: u64,
    pub bump: u8,
}

impl StakeAccount {
    pub const SPACE: usize = 8 + 32 + 8 + 1 + 8 * 3 + 8 + 1;
}

/// Rewards accrued between `last_claim` and `now` at the tier's fixed APY:
///   amount * apy_bps / 10_000 * elapsed / SECONDS_PER_YEAR
/// Computed in u128 to avoid overflow, then narrowed back to u64.
pub fn accrued_reward(amount: u64, tier: u8, last_claim: i64, now: i64) -> Result<u64> {
    if now <= last_claim || amount == 0 {
        return Ok(0);
    }
    let apy_bps = TIER_APY_BPS[tier as usize] as u128;
    let elapsed = (now - last_claim) as u128;
    let reward = (amount as u128)
        .checked_mul(apy_bps)
        .ok_or(StakingError::MathOverflow)?
        .checked_mul(elapsed)
        .ok_or(StakingError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(StakingError::MathOverflow)?
        .checked_div(SECONDS_PER_YEAR as u128)
        .ok_or(StakingError::MathOverflow)?;
    u64::try_from(reward).map_err(|_| StakingError::MathOverflow.into())
}
