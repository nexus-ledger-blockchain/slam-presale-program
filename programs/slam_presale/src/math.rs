use crate::errors::PresaleError;
use anchor_lang::prelude::*;

/// Computes `a * b / denom` using a u128 intermediate so the multiplication
/// never truncates before the division runs.
///
/// This is the exact bug found in the reference implementation this program
/// is based on (microgift/token-presale's `buy.rs`): it computed
/// `(sol_amount / LAMPORTS_PER_SOL) as f64` — integer division *before*
/// converting to float — so any contribution under 1 whole SOL truncated to
/// zero tokens while the SOL was still transferred out. Every conversion in
/// this program must route through this function instead of doing inline
/// division.
pub fn mul_div_u64(a: u64, b: u64, denom: u64) -> Result<u64> {
    require!(denom != 0, PresaleError::MathOverflow);
    let product = (a as u128)
        .checked_mul(b as u128)
        .ok_or(PresaleError::MathOverflow)?;
    let result = product
        .checked_div(denom as u128)
        .ok_or(PresaleError::MathOverflow)?;
    u64::try_from(result).map_err(|_| PresaleError::MathOverflow.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_sol_does_not_truncate_to_zero() {
        // 0.5 SOL (500_000_000 lamports) at a SOL price of $150.00 (micro-usd
        // per SOL = 150_000_000) buying into a round priced at $0.00043/token
        // (430 micro-usd/token) should yield a large, non-zero token amount,
        // not the zero the original repo's bug would have produced.
        let lamports: u64 = 500_000_000;
        let sol_price_micro_usd: u64 = 150_000_000;
        let usd_value_micro = mul_div_u64(lamports, sol_price_micro_usd, 1_000_000_000).unwrap();
        assert_eq!(usd_value_micro, 75_000_000); // 0.5 SOL * $150 = $75.00

        let round_price_micro_usd: u64 = 430;
        let slam_decimals_multiplier: u64 = 1_000_000;
        let tokens = mul_div_u64(usd_value_micro, slam_decimals_multiplier, round_price_micro_usd).unwrap();
        assert!(tokens > 0);
    }

    #[test]
    fn rejects_zero_denominator() {
        assert!(mul_div_u64(100, 100, 0).is_err());
    }

    #[test]
    fn rejects_overflow() {
        assert!(mul_div_u64(u64::MAX, u64::MAX, 1).is_err());
    }
}
