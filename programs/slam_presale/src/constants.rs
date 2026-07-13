/// SLAM uses 6 decimals, matching USDC/USDT. This is a hard constraint, not a
/// style choice: the SPL Token program stores balances as u64 regardless of
/// decimals, and SLAM's fixed 150,000,000,000-token supply at 9 decimals
/// (150e9 * 1e9 = 1.5e20) overflows u64 (max ~1.8447e19). 6 decimals keeps the
/// raw supply (1.5e17) comfortably inside range.
pub const SLAM_DECIMALS: u8 = 6;
pub const SLAM_DECIMALS_MULTIPLIER: u64 = 1_000_000;

/// USDC and USDT both use 6 decimals on Solana, same scale as SLAM, so a
/// stable-coin price can be expressed directly in "USDC/USDT base units per
/// whole SLAM token" with no extra conversion factor.
/// MINIMAL TRANSPARENT RAISE (revised July 13, 2026): a single flat price
/// replaces the old 10-round escalating table. Everyone buys at the same price
/// — no insider discount, no "get in early and flip" dynamic. We keep the
/// round machinery with exactly ONE round so the tested sell-out/hard-cap
/// clamp in `consume_purchase` is reused verbatim rather than rewritten.
pub const NUM_ROUNDS: u8 = 1;

/// The single round's allocation IS the hard cap in tokens: 5,000,000,000 SLAM
/// (3.33% of the 150B supply). 5B × $0.00030 = $1,500,000 hard cap.
pub const ROUND_ALLOCATION_TOKENS: u64 = 5_000_000_000 * SLAM_DECIMALS_MULTIPLIER;

/// Flat price per whole SLAM token in micro-USD (1e6 = $1.00). $0.00030.
/// Priced just below the planned DEX launch price ($0.00035) so early backers
/// aren't sitting on a large discount to dump. Compile-time constant, not
/// admin-adjustable — buyers can trust it can't change after the fact.
pub const ROUND_PRICE_MICRO_USD: [u64; NUM_ROUNDS as usize] = [300];

/// Hard cap in micro-USD ($1,500,000). Redundant with the token allocation
/// above (they must agree) but asserted explicitly at buy time for clarity.
pub const HARD_CAP_MICRO_USD: u64 = 1_500_000_000_000;

/// Soft cap in micro-USD ($200,000). If the sale window closes below this,
/// buyers can reclaim their full contribution via `refund` and no tokens are
/// distributed. Below the audit + minimum-liquidity floor is not worth
/// launching, so the raise fails cleanly rather than half-funding.
pub const SOFT_CAP_MICRO_USD: u64 = 200_000_000_000;

/// Per-transaction minimum ($100) and per-wallet maximum ($25,000). The max
/// spreads distribution and keeps any single buyer from dominating the raise.
pub const MIN_PURCHASE_MICRO_USD: u64 = 100_000_000;
pub const MAX_PER_WALLET_MICRO_USD: u64 = 25_000_000_000;

/// Vesting terms (revised): 20% unlocked at TGE, remaining 80% linearly over
/// ~4 months. Lighter than the old 10%/6mo because the flat-price-≈-launch
/// design already removes the day-one dump incentive.
pub const TGE_UNLOCK_BPS: u64 = 2_000; // 20.00% in basis points
pub const BPS_DENOMINATOR: u64 = 10_000;
pub const VESTING_DURATION_SECONDS: i64 = 120 * 24 * 60 * 60; // ~4 months

pub const GLOBAL_SEED: &[u8] = b"presale-global";
pub const USER_SEED: &[u8] = b"presale-user";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"presale-vault-authority";

pub const PRICE_STALENESS_THRESHOLD_SECONDS: u64 = 60;

/// Maximum number of distinct SPL stable-coin mints (USDC, USDT, ...) this
/// presale can accept. Configured by the admin at `initialize`, not hardcoded,
/// so adding/removing an accepted stablecoin never requires a program upgrade.
pub const MAX_ACCEPTED_STABLES: usize = 4;
