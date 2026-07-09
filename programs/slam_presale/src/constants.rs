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
pub const NUM_ROUNDS: u8 = 10;
pub const ROUND_ALLOCATION_TOKENS: u64 = 750_000_000 * SLAM_DECIMALS_MULTIPLIER; // 750M SLAM per round

/// Price per whole SLAM token in micro-USD (1e6 = $1.00), one entry per round.
/// This matches the approved round table exactly (decided July 9, 2026):
/// R1 $0.00010 .. R10 $0.00097 linear, 750M tokens/round, rounded to integer
/// micro-USD so the sum still lands on the published $4,012,500 total raise.
/// Baked in as a compile-time constant (not admin-adjustable) deliberately:
/// buyers should be able to trust the pricing schedule can't be changed after
/// the fact by whoever holds the admin key.
pub const ROUND_PRICE_MICRO_USD: [u64; NUM_ROUNDS as usize] =
    [100, 197, 293, 390, 487, 583, 680, 777, 873, 970];

/// Minimum contribution per transaction, in micro-USD. Placeholder — confirm
/// the real minimum with the team before deploying; $50 just guards against
/// dust/spam entries in the investor ledger.
pub const MIN_PURCHASE_MICRO_USD: u64 = 50_000_000;

/// Vesting terms, matching the published Tokenomics page: 10% unlocked at
/// TGE, the remaining 90% vesting linearly over 6 months.
pub const TGE_UNLOCK_BPS: u64 = 1_000; // 10.00% in basis points
pub const BPS_DENOMINATOR: u64 = 10_000;
pub const VESTING_DURATION_SECONDS: i64 = 180 * 24 * 60 * 60; // ~6 months

pub const GLOBAL_SEED: &[u8] = b"presale-global";
pub const USER_SEED: &[u8] = b"presale-user";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"presale-vault-authority";

pub const PRICE_STALENESS_THRESHOLD_SECONDS: u64 = 60;

/// Maximum number of distinct SPL stable-coin mints (USDC, USDT, ...) this
/// presale can accept. Configured by the admin at `initialize`, not hardcoded,
/// so adding/removing an accepted stablecoin never requires a program upgrade.
pub const MAX_ACCEPTED_STABLES: usize = 4;
