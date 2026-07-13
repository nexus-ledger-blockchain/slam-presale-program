use crate::constants::*;
use crate::state::PresaleState;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{Mint, Token, TokenAccount};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = PresaleState::SPACE,
        seeds = [GLOBAL_SEED],
        bump
    )]
    pub presale_state: Account<'info, PresaleState>,

    /// PDA that owns `token_vault` and signs SLAM transfers out during claim.
    /// CHECK: only ever used as a signing PDA, never read or written directly.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump)]
    pub vault_authority: AccountInfo<'info>,

    pub slam_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        associated_token::mint = slam_mint,
        associated_token::authority = vault_authority,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    /// CHECK: the destination for SOL/stable-coin proceeds; should be a
    /// multisig wallet. Not validated further here — the admin is trusted at
    /// initialize time, but this address can be changed later via
    /// `update_vault` without redeploying the program.
    pub vault: AccountInfo<'info>,

    /// CHECK: Pyth SOL/USD price account. Not deserialized here (deserializing
    /// as a Pyth price account happens at purchase time); just recorded.
    pub sol_usd_price_feed: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<Initialize>,
    sale_start_ts: i64,
    sale_end_ts: i64,
    accepted_stables: Vec<Pubkey>,
) -> Result<()> {
    require!(
        accepted_stables.len() <= MAX_ACCEPTED_STABLES,
        crate::errors::PresaleError::TooManyStables
    );
    require!(
        sale_end_ts > sale_start_ts,
        crate::errors::PresaleError::InvalidSaleWindow
    );

    let state = &mut ctx.accounts.presale_state;
    state.admin = ctx.accounts.admin.key();
    state.vault = ctx.accounts.vault.key();
    state.slam_mint = ctx.accounts.slam_mint.key();
    state.token_vault = ctx.accounts.token_vault.key();
    state.sol_usd_price_feed = ctx.accounts.sol_usd_price_feed.key();

    let mut stables = [Pubkey::default(); MAX_ACCEPTED_STABLES];
    for (i, mint) in accepted_stables.iter().enumerate() {
        stables[i] = *mint;
    }
    state.accepted_stables = stables;
    state.accepted_stables_len = accepted_stables.len() as u8;

    state.sale_start_ts = sale_start_ts;
    state.sale_end_ts = sale_end_ts;

    state.current_round = 0;
    state.round_tokens_sold = [0; NUM_ROUNDS as usize];
    state.total_tokens_sold = 0;
    state.total_usd_raised_micro = 0;

    state.is_paused = false;
    state.is_finalized = false;
    state.is_claim_active = false;
    state.tge_timestamp = 0;

    state.bump = ctx.bumps.presale_state;
    state.vault_authority_bump = ctx.bumps.vault_authority;

    Ok(())
}
