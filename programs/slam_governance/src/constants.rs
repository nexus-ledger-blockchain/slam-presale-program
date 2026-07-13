// SLAM governance — on-chain, stake-weighted signaling votes. Voting weight is
// read from a voter's stake in the slam_staking program: staked amount times a
// per-tier multiplier (longer locks get more say), matching the published
// governance weights (1x / 1.5x / 2x).

pub const TIER_VOTE_MULT: [u64; 3] = [100, 150, 200]; // 1.0x / 1.5x / 2.0x, in percent

pub const TITLE_MAX: usize = 80;
pub const SUMMARY_MAX: usize = 280;

pub const CONFIG_SEED: &[u8] = b"gov-config";
pub const PROPOSAL_SEED: &[u8] = b"gov-proposal";
pub const VOTE_SEED: &[u8] = b"gov-vote";

// Proposal status
pub const STATUS_ACTIVE: u8 = 0;
pub const STATUS_PASSED: u8 = 1;
pub const STATUS_REJECTED: u8 = 2;

// Vote choice
pub const CHOICE_NO: u8 = 0;
pub const CHOICE_YES: u8 = 1;

// Guard rails on the voting window. The floor exists to reject zero/negative
// periods — those stamp `voting_ends` at or before creation, so a proposal would
// open already closed and never be votable. It is deliberately not a policy
// minimum (that's the admin's call, and tests need short windows).
pub const MIN_VOTING_PERIOD_SECS: i64 = 1;
pub const MAX_VOTING_PERIOD_SECS: i64 = 30 * 24 * 60 * 60; // 30 days
