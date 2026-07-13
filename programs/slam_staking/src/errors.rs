use anchor_lang::prelude::*;

#[error_code]
pub enum StakingError {
    #[msg("Only the admin can perform this action")]
    Unauthorized,
    #[msg("Staking is paused")]
    Paused,
    #[msg("Invalid tier")]
    InvalidTier,
    #[msg("Stake amount is below the minimum")]
    BelowMinimum,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("This account already has an active stake — unstake it first")]
    AlreadyStaked,
    #[msg("No active stake found")]
    NoStake,
    #[msg("Reward vault does not have enough SLAM to cover this claim")]
    InsufficientRewards,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Arithmetic underflow")]
    MathUnderflow,
}
