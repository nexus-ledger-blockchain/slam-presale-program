use crate::constants::*;
use crate::errors::PresaleError;
use crate::state::PresaleState;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

/// Admin-only, callable once the sale window has closed AND the soft cap was
/// reached. Sweeps all escrowed stable proceeds to the multisig vault and marks
/// the raise finalized, which is the precondition for enabling claims. If the
/// soft cap was NOT reached, this fails and buyers refund instead.
#[derive(Accounts)]
pub struct Finalize<'info> {
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = presale_state.bump,
        has_one = admin @ PresaleError::Unauthorized,
    )]
    pub presale_state: Account<'info, PresaleState>,

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

    /// Destination multisig vault's ATA for the stable mint.
    #[account(
        mut,
        associated_token::mint = stable_mint,
        associated_token::authority = presale_state.vault,
    )]
    pub vault_stable_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<Finalize>) -> Result<()> {
    let state = &ctx.accounts.presale_state;
    let now = Clock::get()?.unix_timestamp;

    require!(now > state.sale_end_ts, PresaleError::SaleNotEnded);
    require!(!state.is_finalized, PresaleError::AlreadyFinalized);
    require!(
        state.total_usd_raised_micro >= SOFT_CAP_MICRO_USD,
        PresaleError::SoftCapNotReached
    );

    // Sweep the entire escrow balance to the vault.
    let amount = ctx.accounts.escrow_stable_account.amount;
    if amount > 0 {
        let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[state.vault_authority_bump]];
        let signer: &[&[&[u8]]] = &[seeds];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.escrow_stable_account.to_account_info(),
                    to: ctx.accounts.vault_stable_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer,
            ),
            amount,
        )?;
    }

    ctx.accounts.presale_state.is_finalized = true;
    Ok(())
}
