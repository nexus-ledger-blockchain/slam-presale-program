use crate::constants::*;
use crate::errors::PresaleError;
use crate::state::{PresaleState, UserAllocation};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

/// Buyer-callable once the sale window has closed AND the soft cap was NOT
/// reached (and the raise was never finalized). Returns the buyer's full
/// stable contribution from escrow and zeroes their allocation so it cannot be
/// claimed or refunded twice.
#[derive(Accounts)]
pub struct Refund<'info> {
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

    /// CHECK: signing PDA for the escrow, verified by seeds.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = presale_state.vault_authority_bump)]
    pub vault_authority: AccountInfo<'info>,

    pub stable_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = stable_mint,
        associated_token::authority = vault_authority,
    )]
    pub escrow_stable_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = stable_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_stable_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<Refund>) -> Result<()> {
    let state = &ctx.accounts.presale_state;
    let now = Clock::get()?.unix_timestamp;

    require!(now > state.sale_end_ts, PresaleError::SaleNotEnded);
    require!(!state.is_finalized, PresaleError::SoftCapReached);
    require!(
        state.total_usd_raised_micro < SOFT_CAP_MICRO_USD,
        PresaleError::SoftCapReached
    );

    let amount = ctx.accounts.user_allocation.paid_stable_micro;
    require!(amount > 0, PresaleError::NothingToRefund);

    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[state.vault_authority_bump]];
    let signer: &[&[&[u8]]] = &[seeds];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.escrow_stable_account.to_account_info(),
                to: ctx.accounts.buyer_stable_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        amount,
    )?;

    // Zero the allocation so it can't be refunded again (and has no claimable
    // tokens). total_purchased is cleared too — a refunded buyer holds nothing.
    let alloc = &mut ctx.accounts.user_allocation;
    alloc.paid_stable_micro = 0;
    alloc.total_purchased = 0;

    Ok(())
}
