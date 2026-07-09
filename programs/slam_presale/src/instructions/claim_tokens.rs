use crate::constants::*;
use crate::errors::PresaleError;
use crate::state::{total_vested_amount, PresaleState, UserAllocation};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

#[derive(Accounts)]
pub struct ClaimTokens<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        seeds = [GLOBAL_SEED],
        bump = presale_state.bump,
    )]
    pub presale_state: Account<'info, PresaleState>,

    #[account(
        mut,
        seeds = [USER_SEED, buyer.key().as_ref()],
        bump = user_allocation.bump,
        has_one = buyer @ PresaleError::Unauthorized,
    )]
    pub user_allocation: Account<'info, UserAllocation>,

    /// CHECK: PDA that owns `token_vault`; only ever used to sign the payout
    /// transfer, never read or written directly.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = presale_state.vault_authority_bump)]
    pub vault_authority: AccountInfo<'info>,

    #[account(
        mut,
        address = presale_state.token_vault @ PresaleError::Unauthorized,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    pub slam_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = slam_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_slam_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimTokens>) -> Result<()> {
    let state = &ctx.accounts.presale_state;
    require!(state.is_claim_active, PresaleError::ClaimNotActive);

    let now = Clock::get()?.unix_timestamp;
    let user_allocation = &mut ctx.accounts.user_allocation;

    let vested = total_vested_amount(user_allocation.total_purchased, state.tge_timestamp, now)?;
    let claimable = vested
        .checked_sub(user_allocation.total_claimed)
        .ok_or(PresaleError::MathUnderflow)?;
    require!(claimable > 0, PresaleError::NothingToClaim);
    require!(
        ctx.accounts.token_vault.amount >= claimable,
        PresaleError::InsufficientVaultBalance
    );

    user_allocation.total_claimed = user_allocation
        .total_claimed
        .checked_add(claimable)
        .ok_or(PresaleError::MathOverflow)?;

    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_AUTHORITY_SEED, &[state.vault_authority_bump]]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.token_vault.to_account_info(),
                to: ctx.accounts.buyer_slam_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer_seeds,
        ),
        claimable,
    )?;

    Ok(())
}
