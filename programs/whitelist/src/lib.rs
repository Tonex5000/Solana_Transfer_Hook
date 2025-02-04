use anchor_lang::prelude::*;
use anchor_lang::system_program::{create_account, CreateAccount};
use anchor_spl::{
    token_interface::{Mint, TokenAccount},
};
use solana_program::program_error::ProgramError;
use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList, seeds::Seed};
use spl_transfer_hook_interface::instruction::{ExecuteInstruction, TransferHookInstruction};

declare_id!("F8JTJRsEngZsdw4HkZDHmDWjJtVXUWCPeSgKFondXVbQ");

#[program]
pub mod whitelist_transfer_hook {
    use super::*;

    pub fn initialize_whitelist_state(ctx: Context<InitializeWhitelistState>) -> Result<()> {
        let whitelist_state = &mut ctx.accounts.whitelist_state;
        whitelist_state.allowed_addresses = Vec::new();
        whitelist_state.admin = ctx.accounts.payer.key();
        Ok(())
    }

    pub fn initialize_extra_account_meta_list(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
        
        let account_metas = vec![
            // index 5 - whitelist state account
            ExtraAccountMeta::new_with_seeds(
                &[
                    Seed::Literal {
                        bytes: "whitelist-state".as_bytes().to_vec(),
                    },
                ],
                false,
                true 
            )?,
        ];

        // Calculate account size
        let account_size = ExtraAccountMetaList::size_of(account_metas.len())? as u64;
        let lamports = Rent::get()?.minimum_balance(account_size as usize);

        let mint = ctx.accounts.mint.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"extra-account-metas",
            &mint.as_ref(),
            &[ctx.bumps.extra_account_meta_list]
        ]];

        // Create ExtraAccountMetaList account
        create_account(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                CreateAccount {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.extra_account_meta_list.to_account_info(),
                }
            ).with_signer(signer_seeds),
            lamports,
            account_size,
            ctx.program_id
        )?;

        // Initialize the account data
        ExtraAccountMetaList::init::<ExecuteInstruction>(
            &mut ctx.accounts.extra_account_meta_list.try_borrow_mut_data()?,
            &account_metas
        )?;

        // Initialize whitelist state
        let whitelist_state = &mut ctx.accounts.whitelist_state;
        whitelist_state.is_initialized = true;
        whitelist_state.admin = ctx.accounts.payer.key();

        Ok(())
    }

    pub fn add_to_whitelist(ctx: Context<ManageWhitelist>, address: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.whitelist_state.admin == ctx.accounts.admin.key(),
            WhitelistError::NotAdmin
        );

        ctx.accounts.whitelist_state.allowed_addresses.push(address);
        Ok(())
    }

    pub fn remove_from_whitelist(ctx: Context<ManageWhitelist>, address: Pubkey) -> Result<()> {
        require!(
            ctx.accounts.whitelist_state.admin == ctx.accounts.admin.key(),
            WhitelistError::NotAdmin
        );

        let addresses = &mut ctx.accounts.whitelist_state.allowed_addresses;
        if let Some(index) = addresses.iter().position(|&x| x == address) {
            addresses.remove(index);
        }
        Ok(())
    }

    pub fn transfer_hook(ctx: Context<TransferHook>, _amount: u64) -> Result<()> {
        // Check if sender is whitelisted
        let sender = ctx.accounts.owner.key();
        require!(
            ctx.accounts.whitelist_state.allowed_addresses.contains(&sender),
            WhitelistError::NotWhitelisted
        );
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: ExtraAccountMetaList Account
    #[account(
        mut,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump
    )]
    pub extra_account_meta_list: AccountInfo<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,

    #[account(
        init,
        payer = payer,
        space = WhitelistState::SIZE,
        seeds = [b"whitelist-state"],
        bump
    )]
    pub whitelist_state: Account<'info, WhitelistState>,
}

#[derive(Accounts)]
pub struct ManageWhitelist<'info> {
    pub admin: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"whitelist-state"],
        bump
    )]
    pub whitelist_state: Account<'info, WhitelistState>,
}

#[derive(Accounts)]
pub struct TransferHook<'info> {
    #[account(token::mint = mint, token::authority = owner)]
    pub source_token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(token::mint = mint)]
    pub destination_token: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: source token account owner
    pub owner: UncheckedAccount<'info>,
    
    /// CHECK: ExtraAccountMetaList Account
    #[account(seeds = [b"extra-account-metas", mint.key().as_ref()], bump)]
    pub extra_account_meta_list: UncheckedAccount<'info>,
    
    #[account(seeds = [b"whitelist-state"], bump)]
    pub whitelist_state: Account<'info, WhitelistState>,
}

#[account]
pub struct WhitelistState {
    pub is_initialized: bool,
    pub admin: Pubkey,
    pub allowed_addresses: Vec<Pubkey>,
}

impl WhitelistState {
    pub const SIZE: usize = 8 + // discriminator
        1 + // is_initialized
        32 + // admin
        4 + (32 * 50); // Vec<Pubkey> with capacity for 100 addresses
}

#[derive(Accounts)]
pub struct InitializeWhitelistState<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = WhitelistState::SIZE,
        seeds = [b"whitelist-state"],
        bump
    )]
    pub whitelist_state: Account<'info, WhitelistState>,

    pub system_program: Program<'info, System>,
}


#[error_code]
pub enum WhitelistError {
    #[msg("Not authorized to manage whitelist")]
    NotAdmin,
    #[msg("Address not whitelisted")]
    NotWhitelisted,
}

// Fallback instruction handler
pub fn fallback<'info>(program_id: &Pubkey, accounts: &'info [AccountInfo<'info>], data: &[u8]) -> Result<()> {
    let instruction = TransferHookInstruction::unpack(data)?;
    
    match instruction {
        TransferHookInstruction::Execute { amount } => {
            let amount_bytes = amount.to_le_bytes();
            __private::__global::transfer_hook(program_id, accounts, &amount_bytes)
        }
        _ => Err(ProgramError::InvalidInstructionData.into()),
    }
}
