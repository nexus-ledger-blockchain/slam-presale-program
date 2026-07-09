use crate::constants::*;
use crate::errors::PresaleError;
use crate::math::mul_div_u64;
use crate::state::{PresaleState, UserAllocation};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};
use pyth_sdk_solana::state::SolanaPriceAccount;

#[derive(Accounts)]
pub struct BuyWithSol<'info> {
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

    /// CHECK: must match presale_state.vault; enforced below.
    #[account(mut, address = presale_state.vault @ PresaleError::Unauthorized)]
    pub vault: AccountInfo<'info>,

    /// CHECK: must match presale_state.sol_usd_price_feed; deserialized as a
    /// Pyth price account inside the handler.
    #[account(address = presale_state.sol_usd_price_feed @ PresaleError::InvalidPriceFeed)]
    pub price_feed: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<BuyWithSol>, max_sol_lamports: u64) -> Result<()> {
    require!(max_sol_lamports > 0, PresaleError::ZeroAmount);

    let price_account_info = &ctx.accounts.price_feed;
    let price_feed = SolanaPriceAccount::account_info_to_feed(price_account_info)
        .map_err(|_| PresaleError::InvalidPriceFeed)?;
    let now = Clock::get()?.unix_timestamp;

    let price = price_feed
        .get_price_no_older_than(now, PRICE_STALENESS_THRESHOLD_SECONDS)
        .ok_or(PresaleError::StalePrice)?;
    require!(price.price > 0, PresaleError::InvalidPrice);

    // Pyth prices are `price * 10^expo`. Convert to micro-USD (1e6 = $1.00)
    // per whole SOL generically from `expo`, rather than assuming a fixed
    // exponent like the reference implementation did.
    let sol_usd_price_micro: u64 = {
        let price_u64 = u64::try_from(price.price).map_err(|_| PresaleError::InvalidPrice)?;
        let shift = price.expo + 6;
        if shift >= 0 {
            price_u64
                .checked_mul(10u64.checked_pow(shift as u32).ok_or(PresaleError::MathOverflow)?)
                .ok_or(PresaleError::MathOverflow)?
        } else {
            price_u64
                .checked_div(10u64.checked_pow((-shift) as u32).ok_or(PresaleError::MathOverflow)?)
                .ok_or(PresaleError::MathOverflow)?
        }
    };

    const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
    let usd_value_micro = mul_div_u64(max_sol_lamports, sol_usd_price_micro, LAMPORTS_PER_SOL)?;

    let (tokens, actual_usd_micro) = ctx
        .accounts
        .presale_state
        .consume_purchase(usd_value_micro, now)?;

    // Convert the (possibly clamped) USD amount actually charged back into
    // lamports, so a buyer who hits the tail end of a round only pays for
    // what they received — never the full `max_sol_lamports` they offered.
    let actual_lamports = mul_div_u64(actual_usd_micro, LAMPORTS_PER_SOL, sol_usd_price_micro)?;

    let user_allocation = &mut ctx.accounts.user_allocation;
    if user_allocation.buyer == Pubkey::default() {
        user_allocation.buyer = ctx.accounts.buyer.key();
        user_allocation.bump = ctx.bumps.user_allocation;
    }
    user_allocation.total_purchased = user_allocation
        .total_purchased
        .checked_add(tokens)
        .ok_or(PresaleError::MathOverflow)?;
    user_allocation.paid_sol_lamports = user_allocation
        .paid_sol_lamports
        .checked_add(actual_lamports)
        .ok_or(PresaleError::MathOverflow)?;

    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        ),
        actual_lamports,
    )?;

    emit!(PurchaseEvent {
        buyer: ctx.accounts.buyer.key(),
        tokens,
        usd_value_micro: actual_usd_micro,
        paid_lamports: actual_lamports,
        paid_stable_micro: 0,
    });

    Ok(())
}

#[event]
pub struct PurchaseEvent {
    pub buyer: Pubkey,
    pub tokens: u64,
    pub usd_value_micro: u64,
    pub paid_lamports: u64,
    pub paid_stable_micro: u64,
}
