use anchor_lang::prelude::*;

#[error_code]
pub enum GovError {
    #[msg("Only the admin can perform this action")]
    Unauthorized,
    #[msg("Stake account is not owned by the configured staking program")]
    WrongStakeProgram,
    #[msg("Stake account does not belong to this signer")]
    StakeOwnerMismatch,
    #[msg("Stake account data is malformed")]
    BadStakeData,
    #[msg("You need staked SLAM to do this")]
    NoVotingPower,
    #[msg("Below the minimum voting power required to create a proposal")]
    BelowProposalThreshold,
    #[msg("Title or summary is too long")]
    TextTooLong,
    #[msg("Proposal is not active")]
    NotActive,
    #[msg("Voting has closed")]
    VotingClosed,
    #[msg("Voting is still open")]
    VotingOpen,
    #[msg("Invalid vote choice")]
    InvalidChoice,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Voting period must be between 1 hour and 30 days")]
    InvalidVotingPeriod,
    #[msg("Stake was created after this proposal opened; it cannot vote (anti-recycling snapshot)")]
    StakeTooNew,
}
