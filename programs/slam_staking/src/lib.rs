use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

pub mod constants;
pub mod errors;
pub mod state;

use constants::*;
use errors::StakingError;
use state::*;

declare_id!("FLaXjknGBuX9FYPLzs3CKecYYWVgYuC8nTQequTMxAcH");

#[program]
pub mod slam_staking {
    use super::*;

    /// Step 1 of setup: create the config and the stake vault. The reward
    /// vault is created by `init_rewards` (split across two instructions so
    /// each stays under the BPF stack-frame limit). Admin funds the reward
    /// vault afterward with SLAM from the pre-allocated 30B staking pool.
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let c = &mut ctx.accounts.config;
        c.admin = ctx.accounts.admin.key();
        c.slam_mint = ctx.accounts.slam_mint.key();
        c.stake_vault = ctx.accounts.stake_vault.key();
        c.reward_vault = Pubkey::default(); // set by init_rewards
        c.total_staked = 0;
        c.is_paused = false;
        c.bump = ctx.bumps.config;
        c.vault_authority_bump = ctx.bumps.vault_authority;
        Ok(())
    }

    /// Step 2 of setup: create the reward vault and record it. Admin-only.
    pub fn init_rewards(ctx: Context<InitRewards>) -> Result<()> {
        require!(
            ctx.accounts.config.reward_vault == Pubkey::default(),
            StakingError::AlreadyStaked
        );
        ctx.accounts.config.reward_vault = ctx.accounts.reward_vault.key();
        Ok(())
    }

    /// Stake `amount` SLAM into `tier`. One active stake per wallet: the stake
    /// account is created here and closed on unstake, so a second stake while
    /// one is active is rejected by the account initializer.
    pub fn stake(ctx: Context<Stake>, amount: u64, tier: u8) -> Result<()> {
        require!(!ctx.accounts.config.is_paused, StakingError::Paused);
        require!((tier as usize) < NUM_TIERS, StakingError::InvalidTier);
        require!(amount > 0, StakingError::ZeroAmount);
        require!(amount >= MIN_STAKE_TOKENS, StakingError::BelowMinimum);

        let now = Clock::get()?.unix_timestamp;
        let s = &mut ctx.accounts.stake_account;
        s.owner = ctx.accounts.owner.key();
        s.amount = amount;
        s.tier = tier;
        s.staked_at = now;
        s.lock_end = now
            .checked_add(TIER_LOCK_SECONDS[tier as usize])
            .ok_or(StakingError::MathOverflow)?;
        s.last_claim = now;
        s.reward_claimed = 0;
        s.bump = ctx.bumps.stake_account;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.owner_slam_account.to_account_info(),
                    to: ctx.accounts.stake_vault.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            ),
            amount,
        )?;

        let c = &mut ctx.accounts.config;
        c.total_staked = c.total_staked.checked_add(amount).ok_or(StakingError::MathOverflow)?;
        Ok(())
    }

    /// Claim accrued rewards without unstaking. Resets the accrual clock.
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let s = &ctx.accounts.stake_account;
        require!(s.amount > 0, StakingError::NoStake);

        let reward = accrued_reward(s.amount, s.tier, s.last_claim, now)?;
        if reward > 0 {
            require!(ctx.accounts.reward_vault.amount >= reward, StakingError::InsufficientRewards);
            transfer_from_vault(
                &ctx.accounts.token_program,
                &ctx.accounts.reward_vault,
                &ctx.accounts.owner_slam_account,
                &ctx.accounts.vault_authority,
                ctx.accounts.config.vault_authority_bump,
                reward,
            )?;
            let s = &mut ctx.accounts.stake_account;
            s.last_claim = now;
            s.reward_claimed = s.reward_claimed.checked_add(reward).ok_or(StakingError::MathOverflow)?;
        }
        Ok(())
    }

    /// Unstake principal + accrued rewards and close the stake account. If the
    /// tier is still locked, an early-exit penalty is deducted from principal
    /// and recycled into the reward vault for other stakers.
    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let s = &ctx.accounts.stake_account;
        require!(s.amount > 0, StakingError::NoStake);

        let amount = s.amount;
        let tier = s.tier;
        let reward = accrued_reward(amount, tier, s.last_claim, now)?;

        let early = s.lock_end > now && TIER_LOCK_SECONDS[tier as usize] > 0;
        let penalty = if early {
            (amount as u128)
                .checked_mul(TIER_EARLY_PENALTY_BPS[tier as usize] as u128)
                .and_then(|v| v.checked_div(BPS_DENOMINATOR as u128))
                .and_then(|v| u64::try_from(v).ok())
                .ok_or(StakingError::MathOverflow)?
        } else {
            0
        };
        let principal_out = amount.checked_sub(penalty).ok_or(StakingError::MathUnderflow)?;
        let bump = ctx.accounts.config.vault_authority_bump;

        // Recycle any penalty into the reward vault.
        if penalty > 0 {
            transfer_from_vault(
                &ctx.accounts.token_program,
                &ctx.accounts.stake_vault,
                &ctx.accounts.reward_vault,
                &ctx.accounts.vault_authority,
                bump,
                penalty,
            )?;
        }
        // Return principal.
        transfer_from_vault(
            &ctx.accounts.token_program,
            &ctx.accounts.stake_vault,
            &ctx.accounts.owner_slam_account,
            &ctx.accounts.vault_authority,
            bump,
            principal_out,
        )?;
        // Pay accrued rewards.
        if reward > 0 {
            require!(ctx.accounts.reward_vault.amount >= reward, StakingError::InsufficientRewards);
            transfer_from_vault(
                &ctx.accounts.token_program,
                &ctx.accounts.reward_vault,
                &ctx.accounts.owner_slam_account,
                &ctx.accounts.vault_authority,
                bump,
                reward,
            )?;
        }

        let c = &mut ctx.accounts.config;
        c.total_staked = c.total_staked.checked_sub(amount).ok_or(StakingError::MathUnderflow)?;
        Ok(())
    }

    pub fn set_paused(ctx: Context<AdminOnly>, paused: bool) -> Result<()> {
        ctx.accounts.config.is_paused = paused;
        Ok(())
    }
}

/// Signed transfer out of a program vault, authorized by the vault-authority PDA.
fn transfer_from_vault<'info>(
    token_program: &Program<'info, Token>,
    from: &Account<'info, TokenAccount>,
    to: &Account<'info, TokenAccount>,
    vault_authority: &AccountInfo<'info>,
    bump: u8,
    amount: u64,
) -> Result<()> {
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[bump]];
    let signer: &[&[&[u8]]] = &[seeds];
    token::transfer(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: to.to_account_info(),
                authority: vault_authority.clone(),
            },
            signer,
        ),
        amount,
    )
}

// ─────────────────────────── Accounts ───────────────────────────

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(init, payer = admin, space = StakingConfig::SPACE, seeds = [CONFIG_SEED], bump)]
    pub config: Box<Account<'info, StakingConfig>>,

    /// CHECK: single signing PDA for both vaults.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump)]
    pub vault_authority: AccountInfo<'info>,

    pub slam_mint: Box<Account<'info, Mint>>,

    #[account(
        init,
        payer = admin,
        token::mint = slam_mint,
        token::authority = vault_authority,
        seeds = [STAKE_VAULT_SEED],
        bump
    )]
    pub stake_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitRewards<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ StakingError::Unauthorized,
    )]
    pub config: Box<Account<'info, StakingConfig>>,

    /// CHECK: vault-authority PDA.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = config.vault_authority_bump)]
    pub vault_authority: AccountInfo<'info>,

    #[account(address = config.slam_mint)]
    pub slam_mint: Box<Account<'info, Mint>>,

    #[account(
        init,
        payer = admin,
        token::mint = slam_mint,
        token::authority = vault_authority,
        seeds = [REWARD_VAULT_SEED],
        bump
    )]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, StakingConfig>>,

    #[account(
        init,
        payer = owner,
        space = StakeAccount::SPACE,
        seeds = [STAKE_SEED, owner.key().as_ref()],
        bump
    )]
    pub stake_account: Box<Account<'info, StakeAccount>>,

    #[account(mut, associated_token::mint = config.slam_mint, associated_token::authority = owner)]
    pub owner_slam_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, address = config.stake_vault)]
    pub stake_vault: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, StakingConfig>>,

    #[account(
        mut,
        seeds = [STAKE_SEED, owner.key().as_ref()],
        bump = stake_account.bump,
        has_one = owner @ StakingError::Unauthorized,
    )]
    pub stake_account: Box<Account<'info, StakeAccount>>,

    /// CHECK: vault-authority signing PDA, verified by seeds.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = config.vault_authority_bump)]
    pub vault_authority: AccountInfo<'info>,

    #[account(mut, address = config.reward_vault)]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = config.slam_mint, associated_token::authority = owner)]
    pub owner_slam_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, StakingConfig>>,

    #[account(
        mut,
        seeds = [STAKE_SEED, owner.key().as_ref()],
        bump = stake_account.bump,
        has_one = owner @ StakingError::Unauthorized,
        close = owner,
    )]
    pub stake_account: Box<Account<'info, StakeAccount>>,

    /// CHECK: vault-authority signing PDA, verified by seeds.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = config.vault_authority_bump)]
    pub vault_authority: AccountInfo<'info>,

    #[account(mut, address = config.stake_vault)]
    pub stake_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut, address = config.reward_vault)]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    #[account(mut, associated_token::mint = config.slam_mint, associated_token::authority = owner)]
    pub owner_slam_account: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ StakingError::Unauthorized,
    )]
    pub config: Box<Account<'info, StakingConfig>>,
}
