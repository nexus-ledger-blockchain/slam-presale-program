// SLAM staking — three fixed-APY lock tiers, matching the published Staking
// page. Rewards accrue linearly at the tier's APY and are paid from a
// program-owned reward vault funded from the pre-allocated 30B staking pool.

pub const SLAM_DECIMALS_MULTIPLIER: u64 = 1_000_000; // 6 decimals, like the mint
pub const BPS_DENOMINATOR: u64 = 10_000;
pub const SECONDS_PER_YEAR: u64 = 365 * 24 * 60 * 60;

pub const NUM_TIERS: usize = 3;

/// Lock duration per tier, in seconds. Tier 0 (Flexible) has no lock.
pub const TIER_LOCK_SECONDS: [i64; NUM_TIERS] = [
    0,                    // Flexible
    180 * 24 * 60 * 60,   // 6-month
    365 * 24 * 60 * 60,   // 12-month
];

/// Fixed APY per tier, in basis points (600 = 6.00%). Representative rates
/// inside the published 6–12% ranges.
pub const TIER_APY_BPS: [u64; NUM_TIERS] = [600, 900, 1200]; // 6% / 9% / 12%

/// Early-unstake penalty per tier, in basis points, applied to principal if a
/// locked stake is withdrawn before its lock ends. Flexible has no penalty.
/// Forfeited penalty is recycled into the reward vault for other stakers.
pub const TIER_EARLY_PENALTY_BPS: [u64; NUM_TIERS] = [0, 1000, 1500]; // 0% / 10% / 15%

/// Minimum stake, in whole SLAM (base units applied in the handler).
pub const MIN_STAKE_TOKENS: u64 = 100 * SLAM_DECIMALS_MULTIPLIER; // 100 SLAM

pub const CONFIG_SEED: &[u8] = b"staking-config";
pub const STAKE_SEED: &[u8] = b"stake";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"staking-vault-authority";
// Distinct PDA-seeded token accounts (not ATAs) so both can share the single
// vault-authority PDA — an authority can only hold one ATA per mint.
pub const STAKE_VAULT_SEED: &[u8] = b"staking-stake-vault";
pub const REWARD_VAULT_SEED: &[u8] = b"staking-reward-vault";
