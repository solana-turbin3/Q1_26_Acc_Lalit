#[cfg(test)]
mod tests {

    use {
        anchor_lang::{
            prelude::msg, 
            solana_program::program_pack::Pack, 
            AccountDeserialize, 
            InstructionData, 
            ToAccountMetas
        }, anchor_spl::{
            associated_token::{
                self, 
                spl_associated_token_account
            }, 
            token::spl_token
        }, 
        litesvm::LiteSVM, 
        litesvm_token::{
            spl_token::ID as TOKEN_PROGRAM_ID, 
            CreateAssociatedTokenAccount, 
            CreateMint, MintTo
        }, 
        solana_rpc_client::rpc_client::RpcClient,
        solana_account::Account,
        solana_instruction::Instruction, 
        solana_keypair::Keypair, 
        solana_message::Message, 
        solana_native_token::LAMPORTS_PER_SOL, 
        solana_pubkey::Pubkey, 
        solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID, 
        solana_signer::Signer, 
        solana_transaction::Transaction, 
        solana_address::Address, 
        std::{
            path::PathBuf, 
            str::FromStr
        }
    };

    static PROGRAM_ID: Pubkey = crate::ID;

    // Setup function to initialize LiteSVM and create a payer keypair
    fn setup() -> (LiteSVM, Keypair, Keypair) {
        // Initialize LiteSVM and payer
        let mut program = LiteSVM::new();
        let payer = Keypair::new();
        let taker = Keypair::new();
    
        // Load program SO file
        let so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/deploy/anchor_escrow.so");
    
        let program_data = std::fs::read(so_path).expect("Failed to read program SO file");
    
        program.add_program(PROGRAM_ID, &program_data);

        // Example on how to Load an account from devnet
        // LiteSVM does not have access to real Solana network data since it does not have network access,
        // so we use an RPC client to fetch account data from devnet
        let rpc_client = RpcClient::new("https://api.devnet.solana.com");
        let account_address = Address::from_str("DRYvf71cbF2s5wgaJQvAGkghMkRcp5arvsK2w97vXhi2").unwrap();
        let fetched_account = rpc_client
            .get_account(&account_address)
            .expect("Failed to fetch account from devnet");

        // Set the fetched account in the LiteSVM environment
        // This allows us to simulate interactions with this account during testing
        program.set_account(payer.pubkey(), Account { 
            lamports: fetched_account.lamports, 
            data: fetched_account.data, 
            owner: Pubkey::from(fetched_account.owner.to_bytes()), 
            executable: fetched_account.executable, 
            rent_epoch: fetched_account.rent_epoch 
        }).unwrap();

        msg!("Lamports of fetched account: {}", fetched_account.lamports);

        // Airdrop some SOL to the payer keypair
        program
            .airdrop(&payer.pubkey(), 100 * LAMPORTS_PER_SOL)
            .expect("Failed to airdrop SOL to payer");

        program
            .airdrop(&taker.pubkey(), 100 * LAMPORTS_PER_SOL)
            .expect("Failed to airdrop SOL to taker");
    
        // Return the LiteSVM instance and payer keypair
        (program, payer, taker)
    }

    // Helper function to setup an escrow with make transaction
    fn setup_escrow(program: &mut LiteSVM, payer: &Keypair) -> (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
        let maker = payer.pubkey();
        
        // Create two mints
        let mint_a = CreateMint::new(program, payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        
        let mint_b = CreateMint::new(program, payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();

        // Create maker's ATA for mint A
        let maker_ata_a = CreateAssociatedTokenAccount::new(program, payer, &mint_a)
            .owner(&maker)
            .send()
            .unwrap();

        // Derive escrow and vault PDAs
        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.as_ref(), &123_u64.to_le_bytes()],
            &PROGRAM_ID
        ).0;

        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

        // Mint tokens to maker
        MintTo::new(program, payer, &mint_a, &maker_ata_a, 10_u64.pow(9))
            .send()
            .unwrap();

        // Create and send make transaction
        let make_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: crate::accounts::Make {
                maker,
                mint_a,
                mint_b,
                maker_ata_a,
                escrow,
                vault,
                associated_token_program: spl_associated_token_account::ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: crate::instruction::Make { deposit: 10, seed: 123_u64, receive: 10 }.data(),

        };

        let message = Message::new(&[make_ix], Some(&payer.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[payer], message, recent_blockhash);
        program.send_transaction(transaction).unwrap();

        (maker, mint_a, mint_b, maker_ata_a, escrow, vault)
    }

    fn setup_take(program: &mut LiteSVM, payer: &Keypair, taker: &Keypair, mint_a: &Pubkey, mint_b: &Pubkey, maker_address: &Pubkey) -> (Pubkey, Pubkey, Pubkey) {
        let taker_ata_b = CreateAssociatedTokenAccount::new(program, payer, mint_b)
            .owner(&taker.pubkey())
            .send()
            .unwrap();
        let taker_ata_a = CreateAssociatedTokenAccount::new(program, payer, &mint_a)
            .owner(&taker.pubkey())
            .send()
            .unwrap();
        let maker_ata_b = CreateAssociatedTokenAccount::new(program, payer, &mint_b)
            .owner(maker_address)
            .send()
            .unwrap();

        // Mint tokens to taker
        MintTo::new(program, payer, mint_b, &taker_ata_b, 10_u64.pow(9))
            .send()
            .unwrap();

        (taker_ata_a, taker_ata_b, maker_ata_b)

    }

    /// Helper to run shared setup for each test
    fn setup_all() -> (LiteSVM, Keypair, Keypair, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
        let (mut program, payer, taker) = setup();
        let (maker_address, mint_a, mint_b, maker_ata_a, escrow, vault) = setup_escrow(&mut program, &payer);
        let (taker_ata_a, taker_ata_b, maker_ata_b) = setup_take(&mut program, &payer, &taker, &mint_a, &mint_b, &maker_address);
        (program, payer, taker, maker_address, mint_a, mint_b, maker_ata_a, escrow, vault, taker_ata_a, taker_ata_b, maker_ata_b)
    }

    #[test]
    fn should_create_escrow_and_vault_correctly() {
        let (program, _payer, _taker, maker_address, mint_a, mint_b, _maker_ata_a, escrow, vault, _taker_ata_a, _taker_ata_b, _maker_ata_b) = setup_all();

        let vault_account = program.get_account(&vault).unwrap();
        let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
        assert_eq!(vault_data.amount, 10);
        assert_eq!(vault_data.owner, escrow);
        assert_eq!(vault_data.mint, mint_a);

        let escrow_account = program.get_account(&escrow).unwrap();
        let escrow_data = crate::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
        assert_eq!(escrow_data.seed, 123u64);
        assert_eq!(escrow_data.maker, maker_address);
        assert_eq!(escrow_data.mint_a, mint_a);
        assert_eq!(escrow_data.mint_b, mint_b);
        assert_eq!(escrow_data.receive, 10);
    }

    #[test]
    #[ignore]
    fn should_execute_take_correctly() {
        let (mut program, _payer, taker, maker_address, mint_a, mint_b, _maker_ata_a, escrow, vault, taker_ata_a, taker_ata_b, maker_ata_b) = setup_all();

        let take_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: crate::accounts::Take {
                taker: taker.pubkey(),
                maker: maker_address,
                mint_a,
                mint_b,
                taker_ata_a,
                taker_ata_b,
                maker_ata_b,
                escrow,
                vault,
                associated_token_program: spl_associated_token_account::ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: crate::instruction::Take {}.data(),
        };

        let message = Message::new(&[take_ix], Some(&taker.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&taker], message, recent_blockhash);
        program.send_transaction(transaction).unwrap();
    }

    #[test]
    fn should_fail_when_escrow_is_still_locked() {
        let (mut program, _payer, taker, maker_address, mint_a, mint_b, _maker_ata_a, escrow, vault, _taker_ata_a, taker_ata_b, _maker_ata_b) = setup_all();

        let take_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: crate::accounts::Take {
                taker: taker.pubkey(),
                maker: maker_address,
                mint_a,
                mint_b,
                taker_ata_a: associated_token::get_associated_token_address(&taker.pubkey(), &mint_a),
                taker_ata_b,
                maker_ata_b: associated_token::get_associated_token_address(&maker_address, &mint_b),
                escrow,
                vault,
                associated_token_program: spl_associated_token_account::ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: crate::instruction::Take {}.data(),
        };

        let message = Message::new(&[take_ix], Some(&taker.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&taker], message, recent_blockhash);

        // Should fail with EscrowStillLocked error
        let result = program.send_transaction(transaction);
        assert!(result.is_err(), "Transaction should fail when escrow is locked");
    }

    #[test]
    #[ignore]
    fn should_execute_take_after_5_days_when_timelock_enabled() {
        use anchor_lang::solana_program::clock::Clock;

        let (mut program, _payer, taker, maker_address, mint_a, mint_b, _maker_ata_a, escrow, vault, taker_ata_a, taker_ata_b, maker_ata_b) = setup_all();

        // Time travel 5 days into the future
        let seconds_5_days = (5 * 24 * 60 * 60) + 1;
        let mut clock = program.get_sysvar::<Clock>();
        clock.unix_timestamp += seconds_5_days;
        program.set_sysvar::<Clock>(&clock);

        let take_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: crate::accounts::Take {
                taker: taker.pubkey(),
                maker: maker_address,
                mint_a,
                mint_b,
                taker_ata_a,
                taker_ata_b,
                maker_ata_b,
                escrow,
                vault,
                associated_token_program: spl_associated_token_account::ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: crate::instruction::Take {}.data(),
        };

        let message = Message::new(&[take_ix], Some(&taker.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&taker], message, recent_blockhash);

        match program.send_transaction(transaction) {
            Ok(tx_result) => {
                println!("Take successful after 5 days!");
                for log in &tx_result.logs {
                    println!("{}", log);
                }
            }
            Err(e) => {
                println!("Take failed: {:?}", e);
                panic!("Transaction should succeed after 5 days");
            }
        }
    }

    #[test]
    fn should_refund_tokens_and_close_vault() {
        let (mut program, payer, _taker, maker_address, mint_a, _mint_b, maker_ata_a, escrow, vault, _taker_ata_a, _taker_ata_b, _maker_ata_b) = setup_all();

        // Get initial maker balance
        let maker_ata_a_account = program.get_account(&maker_ata_a).unwrap();
        let maker_ata_a_data = spl_token::state::Account::unpack(&maker_ata_a_account.data).unwrap();
        let initial_balance = maker_ata_a_data.amount;

        // Create and send refund transaction
        let refund_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: crate::accounts::Refund {
                maker: maker_address,
                mint_a,
                maker_ata_a,
                escrow,
                vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: crate::instruction::Refund {}.data(),
        };

        let message = Message::new(&[refund_ix], Some(&payer.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&payer], message, recent_blockhash);

        // Send transaction and ensure it succeeds
        let _tx_result = program.send_transaction(transaction).unwrap();

        // Verify vault is closed
        let vault_account = program.get_account(&vault);
        assert_eq!(vault_account.map(|a| a.lamports).unwrap_or(0), 0, "Vault should have 0 lamports");

        // Verify escrow is closed
        let escrow_account = program.get_account(&escrow);
        println!("Escrow account after refund: {:?}", escrow_account);
        assert_eq!(escrow_account.map(|a| a.lamports).unwrap_or(0), 0, "Escrow should have 0 lamports");

        // Verify maker received tokens back
        let maker_ata_a_account = program.get_account(&maker_ata_a).unwrap();
        let maker_ata_a_data = spl_token::state::Account::unpack(&maker_ata_a_account.data).unwrap();
        assert_eq!(maker_ata_a_data.amount, initial_balance + 10, "Maker should receive refunded tokens");
    }
}