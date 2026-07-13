use anchor_lang::prelude::*;

#[error_code]
pub enum PresaleError {
    #[msg("Presale has not started yet")]
    NotStarted,
    #[msg("Presale has ended")]
    Ended,
    #[msg("Sale end time must be after sale start time")]
    InvalidSaleWindow,
    #[msg("Too many accepted stable-coin mints provided")]
    TooManyStables,
    #[msg("Presale is paused")]
    Paused,
    #[msg("All rounds are sold out")]
    SoldOut,
    #[msg("Contribution amount must be greater than zero")]
    ZeroAmount,
    #[msg("Contribution is below the minimum purchase amount")]
    BelowMinimum,
    #[msg("Unsupported payment token mint")]
    UnsupportedMint,
    #[msg("Provided price feed account does not match the configured SOL/USD feed")]
    InvalidPriceFeed,
    #[msg("Price feed data is stale")]
    StalePrice,
    #[msg("Price feed returned a non-positive price")]
    InvalidPrice,
    #[msg("Only the admin can perform this action")]
    Unauthorized,
    #[msg("Claiming has not been enabled yet")]
    ClaimNotActive,
    #[msg("Claiming has already been enabled")]
    ClaimAlreadyActive,
    #[msg("Nothing available to claim right now")]
    NothingToClaim,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Arithmetic underflow")]
    MathUnderflow,
    #[msg("Vault token account does not have enough SLAM to cover this claim")]
    InsufficientVaultBalance,
    #[msg("Cannot close presale state after purchases have been recorded")]
    StateNotEmpty,
    #[msg("Contribution would exceed the per-wallet maximum")]
    AboveWalletMaximum,
    #[msg("Contribution would exceed the hard cap")]
    AboveHardCap,
    #[msg("The sale window has not closed yet")]
    SaleNotEnded,
    #[msg("The soft cap was reached — the raise succeeded, no refunds")]
    SoftCapReached,
    #[msg("The soft cap was not reached — the raise failed, cannot finalize")]
    SoftCapNotReached,
    #[msg("The raise has already been finalized")]
    AlreadyFinalized,
    #[msg("The raise must be finalized before claiming can be enabled")]
    NotFinalized,
    #[msg("Nothing to refund for this buyer")]
    NothingToRefund,
    #[msg("SOL purchases are disabled for this raise; pay with USDC")]
    SolDisabled,
}
