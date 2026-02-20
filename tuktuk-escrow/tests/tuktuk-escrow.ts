import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TuktukEscrow } from "../target/types/tuktuk_escrow";
import { Keypair, PublicKey } from "@solana/web3.js";
import {
  init,
  taskKey,
  taskQueueAuthorityKey,
} from "@helium/tuktuk-sdk";
import { createMint, getAssociatedTokenAddressSync, getOrCreateAssociatedTokenAccount, mintTo, TOKEN_PROGRAM_ID } from "@solana/spl-token";

describe("tuktuk-escrow",() => {
  // Configure the client to use the local cluster.
  const provider=anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.tuktukEscrow as Program<TuktukEscrow>;
  const connection=provider.connection;
  const providerWallet=provider.wallet as anchor.Wallet;
  const payer=providerWallet.payer;
  let mintA:PublicKey;
  let mintB:PublicKey;
  let makerAtaA:any;
  const seed = new anchor.BN(Math.floor(Math.random() * 1000000));
  const seedBuf=Buffer.alloc(8);
  seedBuf.writeBigUInt64LE(BigInt(seed.toString()));
  let escrowPda:PublicKey;
  let vault:PublicKey;

  const taskQueue = new anchor.web3.PublicKey("84ndxd9T3mrJnEKXCUT9SMTNRbP6ioEAP4qjax9dVjcf");
  const queueAuthority = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("queue_authority")], program.programId)[0];
  const taskQueueAuthority = taskQueueAuthorityKey(taskQueue, queueAuthority)[0];
  
  before(async()=>{

  let tuktukProgram = await init(provider);
    mintA=await createMint(connection,payer,payer.publicKey,null,6);
    mintB =await createMint(connection,payer,payer.publicKey,null,6);
    [escrowPda]=PublicKey.findProgramAddressSync([Buffer.from("escrow"),payer.publicKey.toBuffer(),seedBuf],program.programId);
    vault=getAssociatedTokenAddressSync(mintA,escrowPda,true);
    
    const makerAtaAAddress = getAssociatedTokenAddressSync(mintA, payer.publicKey);
  
  // Create and fund it
  await getOrCreateAssociatedTokenAccount(
    connection, payer, mintA, payer.publicKey
  );
  
  await mintTo(
    connection, payer, mintA, makerAtaAAddress, 
    payer.publicKey, 1_000_000_000
  );
  
  makerAtaA = makerAtaAAddress;
    let txn = await tuktukProgram.methods
        .addQueueAuthorityV0()
        .accounts({
          payer: payer.publicKey,
          taskQueue: taskQueue,
          queueAuthority,
        })
        .rpc();
    
  });
  
  it("Make", async () => {
    const receive=new anchor.BN(1_0000_000);
    const deposit=new anchor.BN(1_000);
    
    const makeTx=await program.methods.make(seed,deposit,receive).accounts({
      maker:payer.publicKey,
      mintA,
      mintB,
      tokenProgram:TOKEN_PROGRAM_ID
    }).rpc()
    
    console.log("Make completed {}",makeTx);
  });
  
  it("Schedule", async () => {
  let tuktukProgram = await init(provider);
  
  let taskId = 10;
  const [taskPda] = taskKey(taskQueue, taskId);
  
 
  try {
    const tx = await program.methods.schedule(taskId).accountsPartial({
      maker: payer.publicKey,
      mintA,
      makerAtaA,
      escrow: escrowPda,
      vault,
      task: taskPda,
      taskQueue,
      queueAuthority,
      systemProgram: anchor.web3.SystemProgram.programId,
      taskQueueAuthority:taskQueueAuthority,
      tuktukProgram:tuktukProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).rpc();
    
    console.log("Schedule completed", tx);
  } catch (error) {
    console.log("\nFull error:", error);
    if (error.logs) {
      console.log("\nTransaction logs:");
      error.logs.forEach(log => console.log(log));
    }
    throw error;
  }
});
});