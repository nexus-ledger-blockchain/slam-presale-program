use crate::errors::PresaleError;
use crate::state::PresaleState;
use crate::constants::*;
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[derive(Accounts)]
pub struct EnableClaim<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = presale_state.bump,
        has_one = admin @ PresaleError::Unauthorized,
    )]
    pub presale_state: Account<'info, PresaleState>,

    #[account(
        address = presale_state.token_vault @ PresaleError::Unauthorized,
    )]
    pub token_vault: Account<'info, TokenAccount>,
}

pub fn handler(ctx: Context<EnableClaim>, tge_timestamp: i64) -> Result<()> {
    let state = &mut ctx.accounts.presale_state;
    require!(!state.is_claim_active, PresaleError::ClaimAlreadyActive);
    require!(
        ctx.accounts.token_vault.amount >= state.total_tokens_sold,
        PresaleError::InsufficientVaultBalance
    );

    state.is_claim_active = true;
    state.tge_timestamp = tge_timestamp;
    Ok(())
}
