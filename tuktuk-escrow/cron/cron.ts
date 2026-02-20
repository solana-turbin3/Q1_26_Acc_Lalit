
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { createCronJob, 
    cronJobTransactionKey, 
    getCronJobForName, 
    init as initCron 
} from "@helium/cron-sdk";
import {
  compileTransaction,
  init,
  taskQueueAuthorityKey
} from "@helium/tuktuk-sdk";
import { 
    LAMPORTS_PER_SOL, 
    PublicKey, 
    SystemProgram, 
    TransactionInstruction 
} from "@solana/web3.js";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { sendInstructions } from "@helium/spl-utils";
import { TuktukEscrow } from "../target/types/tuktuk_escrow";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID } from "@solana/spl-token";

const escrowProgram = anchor.workspace.tuktukEscrow as Program<TuktukEscrow>;

function deriveEscrow(maker:PublicKey,seed:bigint){
    const seedBuf=Buffer.alloc(8);
    seedBuf.writeBigUInt64LE(seed);
    return anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("escrow"),maker.toBuffer(),seedBuf], escrowProgram.programId)[0];
}

async function main() {
    const argv = await yargs(hideBin(process.argv))
        .options({
        cronName: {
            type: "string",
            description: "The name of the cron job to create",
            demandOption: true,
        },
        queueName: {
            type: "string",
            description: "The name of the task queue to use",
            demandOption: true,
        },
        walletPath: {
            type: "string",
            description: "Path to the wallet keypair",
            demandOption: true,
        },
        rpcUrl: {
            type: "string",
            description: "Your Solana RPC URL",
            demandOption: true,
        },
        fundingAmount: {
            type: "number",
            description: "Amount of SOL to fund the cron job with (in lamports)",
            default: 0.01 * LAMPORTS_PER_SOL,
        },
        maker:{
            type:"string",
            description:"The pubkey of the maker",
            default:new PublicKey("HmmMvN4fHhVF6PD5RxWdokVo1YdMTaLz9wunuw3VDBSt")
        },
        escrowSeed:{
            type:"string",
            description:"The seed for the escrow",
            default:BigInt(111)
        },
        mintA:{
            type:"string",
            description:"The pubkey of the mintA",
            default:new PublicKey("ADD A MINT HERE")
        }
        })
        .help()
        .alias("help", "h").argv;

    // Setup connection and provider
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const wallet = provider.wallet as anchor.Wallet;

    console.log("Using wallet:", wallet.publicKey.toBase58());
    console.log("RPC URL:", argv.rpcUrl);
    console.log("Message:", argv.message);

    // Initialize TukTuk program
    const program = await init(provider);
    const cronProgram = await initCron(provider);
    const taskQueue = new anchor.web3.PublicKey("CMreFdKxT5oeZhiX8nWTGz9PtXM1AMYTh6dGR2UzdtrA");

    // Check if task_queue_authority exists for this wallet, if not create it
    const taskQueueAuthorityPda = taskQueueAuthorityKey(taskQueue, wallet.publicKey)[0];
    const taskQueueAuthorityInfo = await provider.connection.getAccountInfo(taskQueueAuthorityPda);
    
    if (!taskQueueAuthorityInfo) {
        console.log("Initializing task queue authority for wallet...");
        await program.methods
            .addQueueAuthorityV0()
            .accounts({
                payer: wallet.publicKey,
                queueAuthority: wallet.publicKey,
                taskQueue,
            })
            .rpc({ skipPreflight: true });
        console.log("Task queue authority initialized!");
    } else {
        console.log("Task queue authority already exists");
    }

    // Check if cron job already exists
    let cronJob = await getCronJobForName(cronProgram, argv.cronName);
    console.log("Cron Job:", cronJob);
    if (!cronJob) {
        console.log("Creating new cron job...");
        const { pubkeys: { cronJob: cronJobPubkey } } = await (await createCronJob(cronProgram, {
            tuktukProgram: program,
            taskQueue,
            args: {
                name: argv.cronName,
                schedule: "0 * * * * *", // Run every minute
                // How many "free" tasks to allocate to this cron job per transaction (whitout paying crank fee)
                // The increment transaction doesn't need to schedule more transactions, so we set this to 0
                freeTasksPerTransaction: 0,
                // We just have one transaction to queue for each cron job, so we set this to 1
                numTasksPerQueueCall: 1,
            }
        }))
        .rpcAndKeys({ skipPreflight: false });
        cronJob = cronJobPubkey;
        console.log("Funding cron job with", argv.fundingAmount / LAMPORTS_PER_SOL, "SOL");
        await sendInstructions(provider, [
        SystemProgram.transfer({
            fromPubkey: provider.publicKey,
            toPubkey: cronJob,
            lamports: argv.fundingAmount,
        }),
        ]);

        const maker=new PublicKey(argv.maker);
        const mintA=new PublicKey(argv.mintA);
        const escrowSeed=BigInt(argv.escrowSeed);
        
        const escrowKey=deriveEscrow(maker,escrowSeed);
        const vault=getAssociatedTokenAddressSync(mintA,escrowKey,true);
        const makerAta=getAssociatedTokenAddressSync(mintA,maker);

        // Create a simple increment instruction
        const autoRefundInstruction = new TransactionInstruction({
            keys: [
                { pubkey: argv.maker, isSigner: false, isWritable: false },
                {pubkey:argv.mintA,isSigner:false,isWritable:false},
                {pubkey:escrowKey,isSigner:false,isWritable:true},
                {pubkey:argv.mintA,isSigner:false,isWritable:false},
                {pubkey:vault,isSigner:false,isWritable:false},
                {pubkey:makerAta,isSigner:false,isWritable:false},
                {pubkey:TOKEN_PROGRAM_ID,isSigner:false,isWritable:false},
                {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
            ],
            data: escrowProgram.coder.instruction.encode("autoRefund", {}),
            programId: escrowProgram.programId,
        });

        // Compile the instruction
        console.log("Compiling instructions...");
        const { transaction, remainingAccounts } = compileTransaction(
            [autoRefundInstruction],
            []
        );

        // Adding increment to the cron job
        await cronProgram.methods
        .addCronTransactionV0({
            index: 0,
            transactionSource: {
            compiledV0: [transaction],
            },
        })
        .accounts({
            payer: provider.publicKey,
            cronJob,
            cronJobTransaction: cronJobTransactionKey(cronJob, 0)[0],
        })
        .remainingAccounts(remainingAccounts)
        .rpc({ skipPreflight: true });
        console.log(`Cron job created!`);
    } else {
        console.log("Cron job already exists");
    }

    console.log("Cron job address:", cronJob.toBase58());
    console.log(`\nYour Auto refund Instruction will be posted every minute. Watch for transactions on task queue ${taskQueue.toBase58()}. To stop the cron job, use the tuktuk-cli:`);
    console.log(`tuktuk -u ${argv.rpcUrl} -w ${argv.walletPath} cron-transaction close --cron-name ${argv.cronName} --id 0`);
    console.log(`tuktuk -u ${argv.rpcUrl} -w ${argv.walletPath} cron close --cron-name ${argv.cronName}`);
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  }); 