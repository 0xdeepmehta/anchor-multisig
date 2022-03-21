import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { MultiSig } from "../target/types/multi_sig";
const assert = require("assert");


describe("multi-sig", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.MultiSig as Program<MultiSig>;

  
  it("Is initialized!", async () => {


    // multiSig account
    const multisig = anchor.web3.Keypair.generate();
    const multisigSize = 1_000_000; // Account size in bytes.

    // Multisig PDA
    const [multisigSigner, nonce] = await anchor.web3.PublicKey.findProgramAddress(
      [multisig.publicKey.toBuffer()],
      program.programId
    );

    const ownerA = anchor.web3.Keypair.generate();
    const ownerB = anchor.web3.Keypair.generate();
    const ownerC = anchor.web3.Keypair.generate();
    const ownerD = anchor.web3.Keypair.generate();

    // multisig owners
    const owners = [ownerA.publicKey, ownerB.publicKey, ownerC.publicKey];

    // No. of signature required for executing txn on behalf of multisig wallet
    const threshold = new anchor.BN(2);

    const tx = await program.rpc.createMultisig(owners, threshold, nonce, {
      accounts: {
        multisig: multisig.publicKey,
      },
      preInstructions: [
        await program.account.multisig.createInstruction(
          multisig,
          multisigSize
        )
      ],
      signers: [multisig]
    });
    console.log("Your transaction signature", tx);

    const a  = await program.provider.connection.getAccountInfo(multisig.publicKey)
    console.log("getAccountInfo :: ", a.data.length)

    // let multisigAccount = await program.account.multisig.fetch(multisig.publicKey);
    let multisigAccountSize = program.account.multisig.size;

    console.log("Multisig Account Size :: ", multisigAccountSize)


    const pid = program.programId;
    const accounts = [
      {
        pubkey: multisig.publicKey,
        isWritable: true,
        isSigner: false,
      },
      {
        pubkey: multisigSigner,
        isWritable: false,
        isSigner: true,
      },
    ];
    const newOwners = [ownerA.publicKey, ownerB.publicKey, ownerD.publicKey];

    const data = program.coder.instruction.encode("set_owners", {
      owners: newOwners,
    });

    console.log("Fuck the new data is look like this :: ", data)

    // New account for storing transaction
    const transaction = anchor.web3.Keypair.generate();
    const txSize = 1000; // Big enough, cuz I'm lazy.

    const createTx = await program.rpc.createTransaction(pid, accounts, data, {
      accounts: {
        multisig: multisig.publicKey,
        transaction: transaction.publicKey,
        proposer: ownerA.publicKey,
      },
      instructions: [
        await program.account.transaction.createInstruction(
          transaction,
          txSize
        ),
      ],
      signers: [transaction, ownerA],
    });

    const txAccount = await program.account.transaction.fetch(
      transaction.publicKey
    );
    console.log("Fuck the new data is look like this :: ", createTx)

    assert.ok(txAccount.programId.equals(pid));
    assert.deepEqual(txAccount.accounts, accounts);
    assert.deepEqual(txAccount.data, data);
    assert.ok(txAccount.multisig.equals(multisig.publicKey));
    assert.equal(txAccount.didExecute, false);


    // Other owner approves transaction.
    await program.rpc.approve({
      accounts: {
        multisig: multisig.publicKey,
        transaction: transaction.publicKey,
        owner: ownerB.publicKey,
      },
      signers: [ownerB],
    });


    // Now that we've reached the threshold, send the transaction.
    const exTx = await program.rpc.executeTransaction({
      accounts: {
        multisig: multisig.publicKey,
        multisigSigner,
        transaction: transaction.publicKey,
      },
      remainingAccounts: program.instruction.setOwners
        .accounts({
          multisig: multisig.publicKey,
          multisigSigner,
        })
        .map((meta) =>
          meta.pubkey.equals(multisigSigner)
            ? { ...meta, isSigner: false }
            : meta
        )
        .concat({
          pubkey: program.programId,
          isWritable: false,
          isSigner: false,
        }),
    });
    console.log("execute Transcation :: ", exTx)

    const multisigAccount = await program.account.multisig.fetch(multisig.publicKey);

    assert.equal(multisigAccount.nonce, nonce);
    assert.ok(multisigAccount.threshold.eq(new anchor.BN(2)));
    assert.deepEqual(multisigAccount.owners, newOwners);


    // const closeTx = await program.rpc.closeAccount({
    //   accounts: {
    //     closeAccount: new anchor.web3.PublicKey("JANyLSuXB9aEBrvvtVnG8a3rwDJf5ztbPep7icTW7JKG"),authority: program.provider.wallet.publicKey,
    //   }
    // });

    // console.log("Your transaction signature", closeTx);

  });
});
