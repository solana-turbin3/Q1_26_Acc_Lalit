use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Minimum time has not passed after make")]
    MinTimeNotPassed,
}
