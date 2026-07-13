use crate::constants::*;
use crate::errors::GovError;
use anchor_lang::prelude::*;

#[account]
pub struct GovConfig {
    pub admin: Pubkey,
    /// The slam_staking program whose StakeAccounts define voting weight.
    pub staking_program: Pubkey,
    pub proposal_count: u64,
    pub voting_period_secs: i64,
    /// Minimum voting weight required to create a proposal.
    pub min_weight_to_propose: u64,
    pub bump: u8,
}
impl GovConfig {
    pub const SPACE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
}

#[account]
pub struct Proposal {
    pub id: u64,
    pub proposer: Pubkey,
    pub title: String,
    pub summary: String,
    pub created_at: i64,
    pub voting_ends: i64,
    pub yes_weight: u64,
    pub no_weight: u64,
    pub status: u8,
    pub bump: u8,
}
impl Proposal {
    pub const SPACE: usize =
        8 + 8 + 32 + (4 + TITLE_MAX) + (4 + SUMMARY_MAX) + 8 + 8 + 8 + 8 + 1 + 1;
}

/// One per (proposal, voter) — its existence prevents double voting.
#[account]
pub struct VoteRecord {
    pub proposal: Pubkey,
    pub voter: Pubkey,
    pub choice: u8,
    pub weight: u64,
    pub bump: u8,
}
impl VoteRecord {
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 8 + 1;
}

/// Reads voting weight from a slam_staking StakeAccount passed as raw account
/// info: verifies the account is owned by the configured staking program and
/// belongs to `expected_owner`, then returns staked_amount * tier multiplier.
pub fn stake_weight(
    stake_ai: &AccountInfo,
    staking_program: &Pubkey,
    expected_owner: &Pubkey,
) -> Result<u64> {
    require_keys_eq!(*stake_ai.owner, *staking_program, GovError::WrongStakeProgram);
    let data = stake_ai.try_borrow_data()?;
    require!(data.len() >= STAKE_MIN_LEN, GovError::BadStakeData);

    let stored_owner = Pubkey::try_from(&data[STAKE_OWNER_OFFSET..STAKE_OWNER_OFFSET + 32])
        .map_err(|_| GovError::BadStakeData)?;
    require_keys_eq!(stored_owner, *expected_owner, GovError::StakeOwnerMismatch);

    let mut amt = [0u8; 8];
    amt.copy_from_slice(&data[STAKE_AMOUNT_OFFSET..STAKE_AMOUNT_OFFSET + 8]);
    let amount = u64::from_le_bytes(amt);

    let tier = data[STAKE_TIER_OFFSET] as usize;
    require!(tier < TIER_VOTE_MULT.len(), GovError::BadStakeData);

    let weight = (amount as u128)
        .checked_mul(TIER_VOTE_MULT[tier] as u128)
        .and_then(|v| v.checked_div(100))
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(GovError::MathOverflow)?;
    Ok(weight)
}
