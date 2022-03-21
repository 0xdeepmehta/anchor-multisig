use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program;


declare_id!("zdt9pYtp3pkKoZh13L97QLfj2iT7GLqPmpRzbtm4a6c");

#[program]
pub mod multi_sig {
    use super::*;

    /// Create a multisig account
    pub fn create_multisig(
        ctx: Context<CreateMultisig>,
        owners: Vec<Pubkey>,
        threshold: u64,
        nonce: u8,
    ) -> Result<()> {
        let multisig = &mut ctx.accounts.multisig;
        multisig.owners = owners;
        multisig.threshold = threshold;
        multisig.nonce = nonce;
        Ok(())
    }

    /// Creatae a tranxsaction account 
    pub fn create_transaction(
        ctx: Context<CreateTransaction>,
        pid: Pubkey,
        accs: Vec<TransactionAccount>, // Accounts required for instruction
        data: Vec<u8>, // Instruction data as u8
    ) -> Result<()> {

        let owner_index = ctx
        .accounts
        .multisig
        .owners
        .iter()
        .position(|a| a == ctx.accounts.proposer.key)
        .ok_or(error!(ErrorCode::InvalidOwner))?;

        let mut signers = Vec::new();
        signers.resize(ctx.accounts.multisig.owners.len(), false);
        signers[owner_index] = true;

        let tx = &mut ctx.accounts.transaction;
        tx.program_id = pid;
        tx.accounts = accs;
        tx.data = data;
        tx.signers = signers;
        tx.multisig = *ctx.accounts.multisig.to_account_info().key;
        tx.did_execute = false;

        Ok(())
    }

    // Approves a transaction on behalf of an owner of the multisig.
    pub fn approve(ctx: Context<Approve>) -> Result<()> {
        let owner_index = ctx
            .accounts
            .multisig
            .owners
            .iter()
            .position(|a| a == ctx.accounts.owner.key)
            .ok_or(error!(ErrorCode::InvalidOwner))?;

        ctx.accounts.transaction.signers[owner_index] = true;

        Ok(())
    }

    // Sets the owners field on the multisig. The only way this can be invoked
    // is via a recursive call from execute_transaction -> set_owners.
    pub fn set_owners(ctx: Context<Auth>, owners: Vec<Pubkey>) -> Result<()> {
        let multisig = &mut ctx.accounts.multisig;

        if (owners.len() as u64) < multisig.threshold {
            multisig.threshold = owners.len() as u64;
        }

        multisig.owners = owners;
        Ok(())
    }

    // Executes the given transaction if threshold owners have signed it.
    pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
        // Has this been executed already?
        if ctx.accounts.transaction.did_execute {
            return err!(ErrorCode::AlreadyExecuted);
        }

        // Do we have enough signers?
        let sig_count = ctx
            .accounts
            .transaction
            .signers
            .iter()
            .filter_map(|s| match s {
                false => None,
                true => Some(true),
            })
            .collect::<Vec<_>>()
            .len() as u64;

        if sig_count < ctx.accounts.multisig.threshold {
            return err!(ErrorCode::NotEnoughSigners);
        }

        // Execute the transcation singed by the multisig.
        let mut ix: Instruction = (&*ctx.accounts.transaction).into();

        ix.accounts = ix
            .accounts
            .iter()
            .map(|acc| {
                if &acc.pubkey == ctx.accounts.multisig_signer.key {
                    AccountMeta::new_readonly(acc.pubkey, true)
                } else {
                    acc.clone()
                }
            })
            .collect();
        let seeds = &[
            ctx.accounts.multisig.to_account_info().key.as_ref(),
            &[ctx.accounts.multisig.nonce],
        ];

        let signer = &[&seeds[..]];
        let accounts = ctx.remaining_accounts;
        solana_program::program::invoke_signed(&ix, &accounts, signer)?;

        // Burn the transaction to ensure one time use.
        ctx.accounts.transaction.did_execute = true;
        Ok(())
    }


    pub fn close_account(_ctx: Context<CloseAccount>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateMultisig<'info> {
    #[account(zero)]
    pub multisig: Account<'info, Multisig>,
}

#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    multisig: Account<'info, Multisig>,

    /// CHECK: Fuck the transcation siger
    #[account(
        seeds = [multisig.to_account_info().key.as_ref()],
        bump = multisig.nonce,
    )]
    multisig_signer: AccountInfo<'info>,

    #[account(mut, has_one = multisig)]
    transaction: Account<'info, Transaction>,
}

#[derive(Accounts)]
pub struct CreateTransaction<'info> {
    pub multisig: Account<'info, Multisig>,

    #[account(zero)]
    pub transaction: Account<'info, Transaction>,

    pub proposer: Signer<'info>
}
#[derive(Accounts)]
pub struct Approve<'info> {
    multisig: Account<'info, Multisig>,
    #[account(mut, has_one = multisig)]
    transaction: Account<'info, Transaction>,
    // One of the multisig owners. Checked in the handler.
    #[account(mut)]
    owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct Auth<'info> {
    #[account(mut)]
    multisig: Account<'info, Multisig>,

    /// CHECK: fuck me hard, 'coz I don't know what i'm doing
    #[account(
        signer,
        seeds = [multisig.to_account_info().key.as_ref()],
        bump = multisig.nonce,
    )]
    multisig_signer: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CloseAccount<'info> {

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut, close=authority)]
    pub close_account: Account<'info, Multisig>,
}

#[account]
#[derive(Default)]
pub struct Multisig {
    owners: Vec<Pubkey>,
    threshold: u64,
    nonce: u8,
}

#[account]
pub struct Transaction {
    // The multisig account this transaction belongs to.
    multisig: Pubkey,

    // Target program to execute against.
    program_id: Pubkey,

    // Accounts required for the transaction.
    accounts: Vec<TransactionAccount>,

    // Instruction data for the transaction.
    data: Vec<u8>,

    // signers[index] is true iff multisig.owners[index] signed the transaction.
    signers: Vec<bool>,

    // Boolean ensuring one time execution.
    did_execute: bool,
}

impl From<&Transaction> for Instruction {
    fn from(tx: &Transaction) -> Instruction {
        Instruction {
            program_id: tx.program_id,
            accounts: tx.accounts.clone().into_iter().map(Into::into).collect(),
            data: tx.data.clone()
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TransactionAccount {
    pubkey: Pubkey,
    is_signer: bool,
    is_writable: bool,
}

impl From<TransactionAccount> for AccountMeta {
    fn from(account: TransactionAccount) -> AccountMeta {
        match account.is_writable {
            false => AccountMeta::new_readonly(account.pubkey, account.is_signer),
            true => AccountMeta::new(account.pubkey, account.is_signer),
        }
    }
}


#[error_code]
pub enum ErrorCode {
    #[msg("The given owner is not part of this multisig.")]
    InvalidOwner,
    #[msg("Not enough owners signed this transaction.")]
    NotEnoughSigners,
    #[msg("Cannot delete a transaction that has been signed by an owner.")]
    TransactionAlreadySigned,
    #[msg("Overflow when adding.")]
    Overflow,
    #[msg("Cannot delete a transaction the owner did not create.")]
    UnableToDelete,
    #[msg("The given transaction has already been executed.")]
    AlreadyExecuted,
    #[msg("Threshold must be less than or equal to the number of owners.")]
    InvalidThreshold,
}
