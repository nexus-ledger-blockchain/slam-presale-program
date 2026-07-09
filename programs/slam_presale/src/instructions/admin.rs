use crate::constants::*;
use crate::errors::PresaleError;
use crate::state::PresaleState;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = presale_state.bump,
        has_one = admin @ PresaleError::Unauthorized,
    )]
    pub presale_state: Account<'info, PresaleState>,
}

pub fn set_paused(ctx: Context<AdminOnly>, paused: bool) -> Result<()> {
    ctx.accounts.presale_state.is_paused = paused;
    Ok(())
}

pub fn update_vault(ctx: Context<AdminOnly>, new_vault: Pubkey) -> Result<()> {
    ctx.accounts.presale_state.vault = new_vault;
    Ok(())
}

pub fn update_price_feed(ctx: Context<AdminOnly>, new_feed: Pubkey) -> Result<()> {
    ctx.accounts.presale_state.sol_usd_price_feed = new_feed;
    Ok(())
}

pub fn update_accepted_stables(ctx: Context<AdminOnly>, mints: Vec<Pubkey>) -> Result<()> {
    require!(
        mints.len() <= MAX_ACCEPTED_STABLES,
        PresaleError::TooManyStables
    );
    let state = &mut ctx.accounts.presale_state;
    let mut stables = [Pubkey::default(); MAX_ACCEPTED_STABLES];
    for (i, mint) in mints.iter().enumerate() {
        stables[i] = *mint;
    }
    state.accepted_stables = stables;
    state.accepted_stables_len = mints.len() as u8;
    Ok(())
}

#[derive(Accounts)]
pub struct TransferAdmin<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = presale_state.bump,
        has_one = admin @ PresaleError::Unauthorized,
    )]
    pub presale_state: Account<'info, PresaleState>,
}

pub fn transfer_admin(ctx: Context<TransferAdmin>, new_admin: Pubkey) -> Result<()> {
    ctx.accounts.presale_state.admin = new_admin;
    Ok(())
}
