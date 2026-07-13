use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod state;

use constants::*;
use errors::GovError;
use slam_staking::state::StakeAccount;
use state::*;

declare_id!("2vM3ByRmDVDV95mWr1PwX3AKyJtT7Cx5WrQWswn87CkV");

#[program]
pub mod slam_governance {
    use super::*;

    /// One-time setup. Records the staking program (source of voting weight),
    /// the voting period, and the minimum weight required to open a proposal.
    pub fn initialize(
        ctx: Context<Initialize>,
        staking_program: Pubkey,
        voting_period_secs: i64,
        min_weight_to_propose: u64,
    ) -> Result<()> {
        // The StakeAccount type already pins voting weight to slam_staking; refuse
        // to record a different program here so the config can't claim otherwise.
        require_keys_eq!(staking_program, slam_staking::ID, GovError::WrongStakeProgram);
        require!(
            (MIN_VOTING_PERIOD_SECS..=MAX_VOTING_PERIOD_SECS).contains(&voting_period_secs),
            GovError::InvalidVotingPeriod
        );

        let c = &mut ctx.accounts.config;
        c.admin = ctx.accounts.admin.key();
        c.staking_program = staking_program;
        c.proposal_count = 0;
        c.voting_period_secs = voting_period_secs;
        c.min_weight_to_propose = min_weight_to_propose;
        c.bump = ctx.bumps.config;
        Ok(())
    }

    /// Open a proposal. Requires the proposer's stake weight to meet the
    /// configured minimum. Proposals are indexed by the config's counter.
    pub fn create_proposal(ctx: Context<CreateProposal>, title: String, summary: String) -> Result<()> {
        require!(title.len() <= TITLE_MAX && summary.len() <= SUMMARY_MAX, GovError::TextTooLong);

        let weight = stake_weight(&ctx.accounts.proposer_stake)?;
        require!(weight >= ctx.accounts.config.min_weight_to_propose, GovError::BelowProposalThreshold);

        let now = Clock::get()?.unix_timestamp;
        let id = ctx.accounts.config.proposal_count;

        let p = &mut ctx.accounts.proposal;
        p.id = id;
        p.proposer = ctx.accounts.proposer.key();
        p.title = title;
        p.summary = summary;
        p.created_at = now;
        p.voting_ends = now.checked_add(ctx.accounts.config.voting_period_secs).ok_or(GovError::MathOverflow)?;
        p.yes_weight = 0;
        p.no_weight = 0;
        p.status = STATUS_ACTIVE;
        p.bump = ctx.bumps.proposal;

        ctx.accounts.config.proposal_count = id.checked_add(1).ok_or(GovError::MathOverflow)?;
        Ok(())
    }

    /// Cast a stake-weighted vote. `choice`: 0 = no, 1 = yes. The vote-record
    /// account is created here, so a second vote on the same proposal fails.
    pub fn cast_vote(ctx: Context<CastVote>, choice: u8) -> Result<()> {
        require!(choice == CHOICE_YES || choice == CHOICE_NO, GovError::InvalidChoice);

        let now = Clock::get()?.unix_timestamp;
        require!(ctx.accounts.proposal.status == STATUS_ACTIVE, GovError::NotActive);
        require!(now <= ctx.accounts.proposal.voting_ends, GovError::VotingClosed);

        // Anti-recycling snapshot: only a stake that already existed when the
        // proposal opened may vote on it. Stake is liquid and weight is sampled
        // at cast time, so without this a single pot of SLAM can be unstaked,
        // moved to a fresh wallet, restaked, and voted again — inflating weight
        // without holding more tokens (verified exploitable at 3x on devnet).
        // A restaked pot always gets a new `staked_at` (stake is `init`, closed
        // on unstake; claim never touches it), so it fails this check.
        require!(
            ctx.accounts.voter_stake.staked_at <= ctx.accounts.proposal.created_at,
            GovError::StakeTooNew
        );

        let weight = stake_weight(&ctx.accounts.voter_stake)?;
        require!(weight > 0, GovError::NoVotingPower);

        let p = &mut ctx.accounts.proposal;
        if choice == CHOICE_YES {
            p.yes_weight = p.yes_weight.checked_add(weight).ok_or(GovError::MathOverflow)?;
        } else {
            p.no_weight = p.no_weight.checked_add(weight).ok_or(GovError::MathOverflow)?;
        }

        let v = &mut ctx.accounts.vote_record;
        v.proposal = p.key();
        v.voter = ctx.accounts.voter.key();
        v.choice = choice;
        v.weight = weight;
        v.bump = ctx.bumps.vote_record;
        Ok(())
    }

    /// Admin-only. Retune the voting window and the proposal threshold without
    /// redeploying. Proposals already open keep the deadline they were created
    /// with — `voting_ends` is stamped at creation, so changing the period never
    /// moves the goalposts on a live vote.
    pub fn set_params(
        ctx: Context<SetParams>,
        voting_period_secs: i64,
        min_weight_to_propose: u64,
    ) -> Result<()> {
        require!(
            (MIN_VOTING_PERIOD_SECS..=MAX_VOTING_PERIOD_SECS).contains(&voting_period_secs),
            GovError::InvalidVotingPeriod
        );
        let c = &mut ctx.accounts.config;
        c.voting_period_secs = voting_period_secs;
        c.min_weight_to_propose = min_weight_to_propose;
        Ok(())
    }

    /// After voting closes, lock in the outcome. Anyone can call. Simple
    /// majority of cast weight; a tie is rejected.
    pub fn finalize(ctx: Context<Finalize>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let p = &mut ctx.accounts.proposal;
        require!(p.status == STATUS_ACTIVE, GovError::NotActive);
        require!(now > p.voting_ends, GovError::VotingOpen);
        p.status = if p.yes_weight > p.no_weight { STATUS_PASSED } else { STATUS_REJECTED };
        Ok(())
    }
}

// ─────────────────────────── Accounts ───────────────────────────

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(init, payer = admin, space = GovConfig::SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Box<Account<'info, GovConfig>>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, GovConfig>>,

    #[account(
        init,
        payer = proposer,
        space = Proposal::SPACE,
        seeds = [PROPOSAL_SEED, config.proposal_count.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Box<Account<'info, Proposal>>,

    /// The proposer's stake. Typed, so Anchor enforces the owning program and
    /// the discriminator; the constraint ties it to this signer.
    #[account(constraint = proposer_stake.owner == proposer.key() @ GovError::StakeOwnerMismatch)]
    pub proposer_stake: Box<Account<'info, StakeAccount>>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetParams<'info> {
    pub admin: Signer<'info>,
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump, has_one = admin @ GovError::Unauthorized)]
    pub config: Box<Account<'info, GovConfig>>,
}

#[derive(Accounts)]
pub struct CastVote<'info> {
    #[account(mut)]
    pub voter: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, GovConfig>>,

    #[account(mut)]
    pub proposal: Box<Account<'info, Proposal>>,

    #[account(
        init,
        payer = voter,
        space = VoteRecord::SPACE,
        seeds = [VOTE_SEED, proposal.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub vote_record: Box<Account<'info, VoteRecord>>,

    /// The voter's stake. Typed (owner program + discriminator checked by
    /// Anchor); the constraint ties it to this signer.
    #[account(constraint = voter_stake.owner == voter.key() @ GovError::StakeOwnerMismatch)]
    pub voter_stake: Box<Account<'info, StakeAccount>>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Finalize<'info> {
    #[account(mut, seeds = [PROPOSAL_SEED, proposal.id.to_le_bytes().as_ref()], bump = proposal.bump)]
    pub proposal: Box<Account<'info, Proposal>>,
}
