use crate::constants::*;
use crate::errors::GovError;
use anchor_lang::prelude::*;
use slam_staking::state::StakeAccount;

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

/// Voting weight for a stake: principal x the lock-tier multiplier.
///
/// The account is a typed `slam_staking::StakeAccount`, so Anchor has already
/// proven it is owned by the staking program and carries the right
/// discriminator; the caller separately checks it belongs to the signer.
///
/// Fields are read by name against staking's own struct, so a layout change
/// there is picked up here automatically on rebuild — unlike hand-rolled byte
/// offsets, which silently mis-read after such a change. The one rule that
/// remains: if StakeAccount's layout changes, **redeploy both programs
/// together**, or a fresh staking binary will not match a stale governance one.
pub fn stake_weight(stake: &StakeAccount) -> Result<u64> {
    let tier = stake.tier as usize;
    require!(tier < TIER_VOTE_MULT.len(), GovError::BadStakeData);

    (stake.amount as u128)
        .checked_mul(TIER_VOTE_MULT[tier] as u128)
        .and_then(|v| v.checked_div(100))
        .and_then(|v| u64::try_from(v).ok())
        .ok_or(GovError::MathOverflow.into())
}
