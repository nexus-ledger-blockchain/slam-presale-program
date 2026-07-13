use crate::constants::*;
use crate::errors::PresaleError;
use crate::state::{PresaleState, UserAllocation};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

#[derive(Accounts)]
pub struct BuyWithStable<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,

    // Heavy accounts are Boxed onto the heap: with two `init_if_needed`
    // accounts (user_allocation + escrow) the generated `try_accounts` stack
    // frame otherwise exceeds the 4096-byte BPF limit.
    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = presale_state.bump,
    )]
    pub presale_state: Box<Account<'info, PresaleState>>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = UserAllocation::SPACE,
        seeds = [USER_SEED, buyer.key().as_ref()],
        bump
    )]
    pub user_allocation: Box<Account<'info, UserAllocation>>,

    /// PDA that owns the escrow (and the SLAM token vault). Signs refunds and
    /// the finalize sweep.
    /// CHECK: signing PDA only, never read or written directly.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = presale_state.vault_authority_bump)]
    pub vault_authority: AccountInfo<'info>,

    /// Stable-coin mint being paid with. Must appear in
    /// `presale_state.accepted_stables` (checked in the handler).
    pub stable_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = stable_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_stable_account: Box<Account<'info, TokenAccount>>,

    /// Program-controlled ESCROW for stable proceeds — the vault_authority PDA's
    /// ATA for this stable mint. Funds sit here during the sale so a failed
    /// (below-soft-cap) raise can be refunded; `finalize` sweeps them to the
    /// multisig vault only once the soft cap is confirmed met.
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = stable_mint,
        associated_token::authority = vault_authority,
    )]
    pub escrow_stable_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
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

    // USDC/USDT use 6 decimals, same scale as SLAM and this program's micro-USD
    // accounting, so the raw token amount IS the micro-USD value.
    let usd_value_micro = stable_amount;
    let now = Clock::get()?.unix_timestamp;

    // Per-wallet maximum: check the buyer's cumulative stable contribution
    // BEFORE consuming, using the pre-clamp requested amount. (The actual
    // charge may be clamped lower by remaining hard-cap capacity, which only
    // makes the true total smaller, never larger.)
    let already_paid = ctx.accounts.user_allocation.paid_stable_micro;
    require!(
        already_paid.saturating_add(usd_value_micro) <= MAX_PER_WALLET_MICRO_USD,
        PresaleError::AboveWalletMaximum
    );

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

    // Proceeds go to the program ESCROW, not the vault — held until finalize.
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.buyer_stable_account.to_account_info(),
                to: ctx.accounts.escrow_stable_account.to_account_info(),
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
