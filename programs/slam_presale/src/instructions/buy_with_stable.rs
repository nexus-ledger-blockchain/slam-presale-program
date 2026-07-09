use crate::constants::*;
use crate::errors::PresaleError;
use crate::state::{PresaleState, UserAllocation};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

#[derive(Accounts)]
pub struct BuyWithStable<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = presale_state.bump,
    )]
    pub presale_state: Account<'info, PresaleState>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = UserAllocation::SPACE,
        seeds = [USER_SEED, buyer.key().as_ref()],
        bump
    )]
    pub user_allocation: Account<'info, UserAllocation>,

    /// Stable-coin mint being paid with (USDC, USDT, ...). Must appear in
    /// `presale_state.accepted_stables`, checked in the handler since the
    /// list length is dynamic.
    pub stable_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = stable_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_stable_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = stable_mint,
        associated_token::authority = presale_state.vault,
    )]
    pub vault_stable_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<BuyWithStable>, stable_amount: u64) -> Result<()> {
    require!(stable_amount > 0, PresaleError::ZeroAmount);

    let state = &ctx.accounts.presale_state;
    let mint_key = ctx.accounts.stable_mint.key();
    let is_accepted = state.accepted_stables[..state.accepted_stables_len as usize]
        .iter()
        .any(|m| *m == mint_key);
    require!(is_accepted, PresaleError::UnsupportedMint);

    // USDC/USDT both use 6 decimals on Solana — the same scale as SLAM_DECIMALS
    // and as this program's micro-USD accounting — so the raw token amount
    // *is* the micro-USD value with no extra conversion.
    let usd_value_micro = stable_amount;
    let now = Clock::get()?.unix_timestamp;

    let (tokens, actual_usd_micro) = ctx
        .accounts
        .presale_state
        .consume_purchase(usd_value_micro, now)?;

    let user_allocation = &mut ctx.accounts.user_allocation;
    if user_allocation.buyer == Pubkey::default() {
        user_allocation.buyer = ctx.accounts.buyer.key();
        user_allocation.bump = ctx.bumps.user_allocation;
    }
    user_allocation.total_purchased = user_allocation
        .total_purchased
        .checked_add(tokens)
        .ok_or(PresaleError::MathOverflow)?;
    user_allocation.paid_stable_micro = user_allocation
        .paid_stable_micro
        .checked_add(actual_usd_micro)
        .ok_or(PresaleError::MathOverflow)?;

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.buyer_stable_account.to_account_info(),
                to: ctx.accounts.vault_stable_account.to_account_info(),
                authority: ctx.accounts.buyer.to_account_info(),
            },
        ),
        actual_usd_micro,
    )?;

    emit!(super::buy_with_sol::PurchaseEvent {
        buyer: ctx.accounts.buyer.key(),
        tokens,
        usd_value_micro: actual_usd_micro,
        paid_lamports: 0,
        paid_stable_micro: actual_usd_micro,
    });

    Ok(())
}
