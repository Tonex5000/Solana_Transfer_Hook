import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { WhitelistTransferHook } from "../target/types/whitelist_transfer_hook";
import {
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  createTransferCheckedWithTransferHookInstruction,
  ExtensionType,
  getMintLen,
  createInitializeMintInstruction,
  createInitializeTransferHookInstruction,
} from "@solana/spl-token";
import { 
  Keypair, 
  PublicKey, 
  SystemProgram, 
  Transaction,
  sendAndConfirmTransaction
} from "@solana/web3.js";
import { expect } from "chai";
import * as path from 'path';
import * as fs from 'fs';
//import chaiAsPromised from 'chai-as-promised';

//chai.use(chaiAsPromised);

describe("whitelist-transfer-hook", () => {
  // Configuration remains the same...
  const walletKeyPath = path.join(__dirname, '../funded_wallet.json');
  const walletKeypair = anchor.web3.Keypair.fromSecretKey(
    Buffer.from(JSON.parse(fs.readFileSync(walletKeyPath, 'utf-8')))
  );
  const fundedWallet = new anchor.Wallet(walletKeypair);
  const provider = new anchor.AnchorProvider(
    anchor.AnchorProvider.env().connection, 
    fundedWallet, 
    { commitment: 'confirmed' }
  );
  anchor.setProvider(provider);

  const program = anchor.workspace.WhitelistTransferHook as Program<WhitelistTransferHook>;
  const payer = provider.wallet as anchor.Wallet;
  const recipient = Keypair.generate();
  const whitelistedUser = Keypair.generate();
  const nonWhitelistedUser = Keypair.generate();

  let mint: Keypair;
  let payerTokenAccount: PublicKey;
  let recipientTokenAccount: PublicKey;
  let extraAccountMetaList: PublicKey;
  let whitelistStatePDA: PublicKey;

  before(async () => {
    mint = Keypair.generate();
    [extraAccountMetaList] = PublicKey.findProgramAddressSync(
      [Buffer.from("extra-account-metas"), mint.publicKey.toBuffer()],
      program.programId
    );
    [whitelistStatePDA] = PublicKey.findProgramAddressSync(
      [Buffer.from("whitelist-state")],
      program.programId
    );
  });

  it("Initializes the mint with transfer hook", async () => {
    const decimals = 0;
    const extensions = [ExtensionType.TransferHook];
    const mintLen = getMintLen(extensions);
    const lamports = await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: mint.publicKey,
        space: mintLen,
        lamports,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferHookInstruction(
        mint.publicKey,
        payer.publicKey,
        program.programId,
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        mint.publicKey,
        decimals,
        payer.publicKey,
        null,
        TOKEN_2022_PROGRAM_ID
      )
    );

    await sendAndConfirmTransaction(
      provider.connection,
      transaction,
      [payer.payer, mint],
      { skipPreflight: true }
    );

    // Create token accounts
    const payerATA = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer.payer,
      mint.publicKey,
      payer.publicKey,
      false,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    payerTokenAccount = payerATA.address;

    const recipientATA = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer.payer,
      mint.publicKey,
      recipient.publicKey,
      false,
      undefined,
      undefined,
      TOKEN_2022_PROGRAM_ID
    );
    recipientTokenAccount = recipientATA.address;
  });

  // Rest of the tests remain the same...
  it("Adds addresses to whitelist", async () => {
    // Add whitelisted user to the whitelist
    await program.methods
      .addToWhitelist(whitelistedUser.publicKey)
      .accounts({
        admin: payer.publicKey,
        whitelistState: whitelistStatePDA,
      })
      .rpc();

    // Fetch and verify the whitelist state
    const whitelistState = await program.account.whitelistState.fetch(whitelistStatePDA);
    expect(whitelistState.allowedAddresses).to.deep.include.members([whitelistedUser.publicKey]);
  });


  it("Removes address from whitelist", async () => {
    // Remove whitelisted user from the whitelist
    await program.methods
      .removeFromWhitelist(whitelistedUser.publicKey)
      .accounts({
        admin: payer.publicKey,
        whitelistState: whitelistStatePDA,
      })
      .rpc();

    // Fetch and verify the whitelist state
    const whitelistState = await program.account.whitelistState.fetch(whitelistStatePDA);
    expect(whitelistState.allowedAddresses).to.not.deep.include.members([whitelistedUser.publicKey]);
  });

});