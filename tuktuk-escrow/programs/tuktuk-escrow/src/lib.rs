#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;
mod error;
mod instructions;
mod state;

use instructions::*;
declare_id!("GQcUgjuH7CXKXC6N7w4T5xyLYCUbSa2tCe2goRqtxjio");

#[program]
pub mod tuktuk_escrow {
    use super::*;
    pub fn make(ctx: Context<Make>, seed: u64, deposit: u64, receive: u64) -> Result<()> {
        ctx.accounts.init_escrow(seed, receive, &ctx.bumps)?;
        ctx.accounts.deposit(deposit)
    }

    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        ctx.accounts.refund_and_close_vault()
    }

    pub fn take(ctx: Context<Take>) -> Result<()> {
        ctx.accounts.validate_time()?;
        ctx.accounts.deposit()?;
        ctx.accounts.withdraw_and_close_vault()
    }

    pub fn auto_refund(ctx: Context<AutoRefund>) -> Result<()> {
        ctx.accounts.refund_and_close_vault()
    }

    pub fn schedule(ctx: Context<Schedule>, task_id: u16) -> Result<()> {
        ctx.accounts.schedule(task_id, ctx.bumps)
    }
}
