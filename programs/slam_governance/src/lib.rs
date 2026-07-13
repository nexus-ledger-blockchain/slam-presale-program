use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod state;

use constants::*;
use errors::GovError;
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

        let weight = stake_weight(
            &ctx.accounts.proposer_stake,
            &ctx.accounts.config.staking_program,
            &ctx.accounts.proposer.key(),
        )?;
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

        let weight = stake_weight(
            &ctx.accounts.voter_stake,
            &ctx.accounts.config.staking_program,
            &ctx.accounts.voter.key(),
        )?;
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

    /// CHECK: the proposer's slam_staking StakeAccount; validated in
    /// `stake_weight` (owner program + stored owner) rather than by type, to
    /// avoid a hard dependency on the staking crate.
    pub proposer_stake: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
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

    /// CHECK: the voter's slam_staking StakeAccount; validated in `stake_weight`.
    pub voter_stake: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Finalize<'info> {
    #[account(mut, seeds = [PROPOSAL_SEED, proposal.id.to_le_bytes().as_ref()], bump = proposal.bump)]
    pub proposal: Box<Account<'info, Proposal>>,
}
