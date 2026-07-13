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

// Byte offsets into a slam_staking StakeAccount (after the 8-byte Anchor
// discriminator): owner:Pubkey@8, amount:u64@40, tier:u8@48. Read manually so
// governance doesn't take a hard dependency on the staking crate.
pub const STAKE_OWNER_OFFSET: usize = 8;
pub const STAKE_AMOUNT_OFFSET: usize = 40;
pub const STAKE_TIER_OFFSET: usize = 48;
pub const STAKE_MIN_LEN: usize = 49;
