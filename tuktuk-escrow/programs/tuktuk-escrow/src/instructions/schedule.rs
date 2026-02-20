use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::{prelude::*, InstructionData};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use tuktuk_program::{
    compile_transaction,
    tuktuk::{
        cpi::{accounts::QueueTaskV0, queue_task_v0},
        program::Tuktuk,
        types::TriggerV0,
    },
    types::QueueTaskArgsV0,
    TransactionSourceV0,
};

use crate::instruction::AutoRefund;
use crate::state::Escrow;

#[derive(Accounts)]
pub struct Schedule<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    mint_a: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
    )]
    maker_ata_a: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        close = maker,
        has_one = mint_a,
        has_one = maker,
        seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
    )]
    vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    /// CHECK: Don't need to parse this account, just using it in CPI
    pub task_queue: UncheckedAccount<'info>,
    /// CHECK: Don't need to parse this account, just using it in CPI
    pub task_queue_authority: UncheckedAccount<'info>,
    /// CHECK: Initialized in CPI
    #[account(mut)]
    pub task: UncheckedAccount<'info>,
    /// CHECK: Via seeds
    #[account(
        mut,
        seeds = [b"queue_authority"],
        bump
    )]
    pub queue_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub tuktuk_program: Program<'info, Tuktuk>,
}

impl<'info> Schedule<'info> {
    pub fn schedule(&mut self, task_id: u16, bumps: ScheduleBumps) -> Result<()> {
        let (compiled_tx, _) = compile_transaction(
            vec![Instruction {
                program_id: crate::ID,
                accounts: crate::__cpi_client_accounts_auto_refund::AutoRefund {
                    escrow: self.escrow.to_account_info(),
                    maker: self.maker.to_account_info(),
                    maker_ata_a: self.maker_ata_a.to_account_info(),
                    mint_a: self.mint_a.to_account_info(),
                    system_program: self.system_program.to_account_info(),
                    token_program: self.token_program.to_account_info(),
                    vault: self.vault.to_account_info(),
                }
                .to_account_metas(None)
                .to_vec(),
                data: AutoRefund {}.data(),
            }],
            vec![],
        )
        .unwrap();

        queue_task_v0(
            CpiContext::new_with_signer(
                self.tuktuk_program.to_account_info(),
                QueueTaskV0 {
                    payer: self.maker.to_account_info(),
                    queue_authority: self.queue_authority.to_account_info(),
                    task_queue_authority: self.task_queue_authority.to_account_info(),
                    task_queue: self.task_queue.to_account_info(),
                    task: self.task.to_account_info(),
                    system_program: self.system_program.to_account_info(),
                },
                &[&["queue_authority".as_bytes(), &[bumps.queue_authority]]],
            ),
            QueueTaskArgsV0 {
                id: task_id,
                trigger: TriggerV0::Now,
                transaction: TransactionSourceV0::CompiledV0(compiled_tx),
                crank_reward: Some(1000001),
                free_tasks: 0,
                description: "Test".to_string(),
            },
        )?;

        Ok(())
    }
}
