import {
  AccountRole,
  Address,
  createAddressWithSeed,
  TransactionSigner,
  SolanaRpcResponse,
  AccountInfoBase,
} from '@solana/kit';
import { getCreateAccountWithSeedInstruction } from '@solana-program/system';
import { Buffer } from 'buffer';

import { DepositArgs, WithdrawArgs, ParsedNewInstrumentArgs, SwapArgs, Instrument, Instruction } from '../types';
import { AccountType } from '../types/enums';
import {
  SYSTEM_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  ADDRESS_LOOKUP_TABLE_PROGRAM_ID,
  DF,
  STANDARD_MAPS_SIZE,
  EXTENDED_MAPS_SIZE,
} from '../constants';
import { findAssociatedTokenAddress, getLookupTableAddress, tokenDec } from './utils';
import {
  getAccountByTag,
  getInstrAccountByTag,
  getTokenAccount,
  getProgramTokenAccount,
  getTokenId,
  getInstrId,
  findClientPrimaryAccount,
  findClientCommunityAccount,
  requireClientCommunityAccount,
  requireClientPrimaryAccount,
} from './account-helpers';
import { InstrFlag } from '../structure_models';
import { depositData, withdrawData, newInstrumentData, swapData } from '../instruction_models';
import { SpotInstructionContext } from './spot-instructions';
import { PerpInstructionContext } from './perp-instructions';
import { getSpotOneSidedContext } from './context-builders';

/**
 * Build deposit instruction
 */
async function buildDepositInstruction(
  ctx: SpotInstructionContext,
  args: DepositArgs,
  exists: boolean,
  rpcGetSlot: () => Promise<bigint>,
): Promise<Instruction> {
  const amount = args.amount ?? 0;
  const allFunds = args.all_funds ? 1 : 0;
  const token = ctx.tokens.get(args.tokenId);
  if (!token) {
    throw new Error(`Token ${args.tokenId} not found`);
  }
  const tokenProgramId = (token.mask & 0x80000000) != 0 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
  const clientTokenAccount = await findAssociatedTokenAddress(ctx.signer, tokenProgramId, token.address);
  const clientPrimaryAccount = await findClientPrimaryAccount(ctx, ctx.signer);

  let keys = [
    { address: ctx.signer, role: AccountRole.READONLY_SIGNER },
    { address: clientTokenAccount, role: AccountRole.WRITABLE },
    { address: token.programAddress, role: AccountRole.WRITABLE },
    { address: token.address, role: AccountRole.READONLY },
    { address: ctx.rootAccount, role: exists ? AccountRole.READONLY : AccountRole.WRITABLE },
    { address: await getTokenAccount(ctx, token.address), role: AccountRole.READONLY },
    { address: clientPrimaryAccount, role: AccountRole.WRITABLE },
    { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
    { address: tokenProgramId, role: AccountRole.READONLY },
  ];

  if (ctx.privateMode) {
    keys.push({
      address: await getAccountByTag(ctx, AccountType.PRIVATE_CLIENTS),
      role: AccountRole.WRITABLE,
    });
  }

  if (exists) {
    if (args.tokenId == 0) {
      keys.push({
        address: await getAccountByTag(ctx, AccountType.COMMUNITY),
        role: AccountRole.WRITABLE,
      });
      keys.push({ address: requireClientCommunityAccount(ctx), role: AccountRole.WRITABLE });
    }
    return {
      accounts: keys,
      programAddress: ctx.programId,
      data: depositData(7, 0, allFunds, args.tokenId, amount * tokenDec(ctx.tokens, args.tokenId, ctx.uiNumbers), 0, 0, args.customId ?? 0),
    };
  } else {
    const slot = Number(await rpcGetSlot()) - 1;
    const lutAddress = await getLookupTableAddress(ctx.signer, slot);
    const clientCommunityAccount = await findClientCommunityAccount(ctx, ctx.signer);
    keys.push({ address: clientCommunityAccount, role: AccountRole.WRITABLE });
    keys.push({ address: lutAddress, role: AccountRole.WRITABLE });
    keys.push({ address: ADDRESS_LOOKUP_TABLE_PROGRAM_ID, role: AccountRole.WRITABLE });
    if (args.tokenId == 0) {
      keys.push({
        address: await getAccountByTag(ctx, AccountType.COMMUNITY),
        role: AccountRole.WRITABLE,
      });
      keys.push({ address: clientCommunityAccount, role: AccountRole.WRITABLE });
    }
    let refId: number;
    if (args.refId != null && args.refId != undefined) {
      refId = args.refId;
      if (args.refWallet == null) {
        throw new Error('Ref Wallet Not Found');
      }
      keys.push({
        address: await findClientPrimaryAccount(ctx, args.refWallet),
        role: AccountRole.WRITABLE,
      });
      keys.push({
        address: await findClientCommunityAccount(ctx, args.refWallet),
        role: AccountRole.WRITABLE,
      });
    } else {
      refId = 0;
    }
    return {
      accounts: keys,
      programAddress: ctx.programId,
      data: depositData(
        7,
        0,
        allFunds,
        args.tokenId,
        amount * tokenDec(ctx.tokens, args.tokenId, ctx.uiNumbers),
        slot,
        refId,
        args.customId ?? 0,
      ),
    };
  }
}

/**
 * Build withdraw instruction
 */
async function buildWithdrawInstruction(ctx: SpotInstructionContext, args: WithdrawArgs): Promise<Instruction> {
  const token = ctx.tokens.get(args.tokenId);
  if (!token) {
    throw new Error(`Token ${args.tokenId} not found`);
  }
  const tokenProgramId = (token.mask & 0x80000000) != 0 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
  const clientTokenAccount = await findAssociatedTokenAddress(ctx.signer, tokenProgramId, token.address);

  const clientPrimaryAccount = requireClientPrimaryAccount(ctx);

  let keys = [
    { address: ctx.signer, role: AccountRole.WRITABLE_SIGNER },
    { address: clientTokenAccount, role: AccountRole.WRITABLE },
    { address: token.programAddress, role: AccountRole.WRITABLE },
    { address: token.address, role: AccountRole.READONLY },
    { address: ctx.rootAccount, role: args.tokenId === 0 ? AccountRole.WRITABLE : AccountRole.READONLY },
    { address: await getTokenAccount(ctx, token.address), role: AccountRole.READONLY },
    { address: clientPrimaryAccount, role: AccountRole.WRITABLE },
    { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
    { address: tokenProgramId, role: AccountRole.READONLY },
    { address: ASSOCIATED_TOKEN_PROGRAM_ID, role: AccountRole.WRITABLE },
  ];

  if (args.spot != undefined) {
    for (let i = 0; i < args.spot.length; ++i) {
      const instr = ctx.instruments.get(args.spot[i].instrId);
      if (!instr) {
        throw new Error(`Instrument ${args.spot[i].instrId} not found`);
      }
      if (instr.header.assetTokenId == args.tokenId || instr.header.crncyTokenId == args.tokenId) {
        keys.push({ address: instr.header.mapsAddress, role: AccountRole.WRITABLE });
        keys.push({
          address: await getInstrAccountByTag(ctx, {
            assetTokenId: instr.header.assetTokenId,
            crncyTokenId: instr.header.crncyTokenId,
            tag: AccountType.SPOT_CLIENT_INFOS,
          }),
          role: AccountRole.WRITABLE,
        });
      }
    }
  }

  if (args.tokenId == 0) {
    keys.push({
      address: await getAccountByTag(ctx, AccountType.COMMUNITY),
      role: AccountRole.WRITABLE,
    });
    keys.push({ address: requireClientCommunityAccount(ctx), role: AccountRole.WRITABLE });
  }

  return {
    accounts: keys,
    programAddress: ctx.programId,
    data: withdrawData(8, args.tokenId, args.amount * tokenDec(ctx.tokens, args.tokenId, ctx.uiNumbers), args.customId ?? 0),
  };
}

/**
 * Build new instrument instructions
 */
async function buildNewInstrumentInstructions(
  ctx: PerpInstructionContext,
  args: ParsedNewInstrumentArgs,
  rpcGetSlot: () => Promise<bigint>,
  rpcGetAccountInfo: (address: Address) => Promise<SolanaRpcResponse<AccountInfoBase | null>>,
  rpcGetMinBalance: (size: bigint) => Promise<bigint>,
): Promise<Instruction[]> {
  if (args.initialPrice <= 0) {
    throw new Error('Invalid initial price');
  }

  const assetInfo = await rpcGetAccountInfo(args.assetMint);
  if (!assetInfo.value) {
    throw new Error('Asset mint not found');
  }

  const tokenProgramId = assetInfo.value.owner == TOKEN_2022_PROGRAM_ID ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
  const crncyTokenId = await getTokenId(ctx, args.crncyMint);
  const id = await getTokenId(ctx, args.assetMint);
  const newAssetToken = id == null;
  const assetTokenId = newAssetToken ? ctx.rootStateModel.tokensCount : id;

  if (!crncyTokenId) {
    throw new Error('Currency mint not found');
  }

  const mapsAccountSeed =
    ctx.version.toString() +
    '_' +
    AccountType.SPOT_MAPS.toString() +
    '_' +
    assetTokenId.toString() +
    '_' +
    crncyTokenId.toString();
  const mapsAccount = await createAddressWithSeed({
    baseAddress: ctx.signer,
    programAddress: ctx.programId,
    seed: mapsAccountSeed,
  });
  const mapsAccountSize = (args.mask & InstrFlag.similarAssets) !== 0 ? EXTENDED_MAPS_SIZE : STANDARD_MAPS_SIZE;
  const mapsAccountLamports = await rpcGetMinBalance(BigInt(mapsAccountSize));
  const createMapsAccountIx = getCreateAccountWithSeedInstruction({
    payer: ctx.signer as unknown as TransactionSigner,
    baseAccount: ctx.signer as unknown as TransactionSigner,
    base: ctx.signer,
    newAccount: mapsAccount,
    seed: mapsAccountSeed,
    space: mapsAccountSize,
    programAddress: ctx.programId,
    amount: mapsAccountLamports,
  });

  const slot = Number(await rpcGetSlot()) - 1;
  const lutAddress = await getLookupTableAddress(ctx.drvsAuthority, slot);

  const tokenArgs = { assetTokenId: assetTokenId, crncyTokenId: crncyTokenId };

  let keys = [
    { address: ctx.signer, role: AccountRole.READONLY_SIGNER },
    { address: ctx.rootAccount, role: AccountRole.WRITABLE },
    {
      address: await getTokenAccount(ctx, args.assetMint),
      role: newAssetToken ? AccountRole.WRITABLE : AccountRole.READONLY,
    },
    { address: await getTokenAccount(ctx, args.crncyMint), role: AccountRole.READONLY },
    {
      address: newAssetToken
        ? await getProgramTokenAccount(ctx, args.assetMint)
        : ctx.tokens.get(assetTokenId)!.programAddress,
      role: newAssetToken ? AccountRole.WRITABLE : AccountRole.READONLY,
    },
    { address: args.assetMint, role: AccountRole.READONLY },
    { address: lutAddress, role: AccountRole.WRITABLE },
    { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
    { address: tokenProgramId, role: AccountRole.READONLY },
    { address: ADDRESS_LOOKUP_TABLE_PROGRAM_ID, role: AccountRole.READONLY },
    { address: ctx.drvsAuthority, role: AccountRole.READONLY },
    {
      address: await getInstrAccountByTag(ctx, { ...tokenArgs, tag: AccountType.INSTR }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...tokenArgs, tag: AccountType.SPOT_BIDS_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...tokenArgs, tag: AccountType.SPOT_ASKS_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...tokenArgs, tag: AccountType.SPOT_BID_ORDERS }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...tokenArgs, tag: AccountType.SPOT_ASK_ORDERS }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...tokenArgs, tag: AccountType.SPOT_LINES }),
      role: AccountRole.WRITABLE,
    },
    { address: mapsAccount, role: AccountRole.WRITABLE },
    {
      address: await getInstrAccountByTag(ctx, {
        ...tokenArgs,
        tag: AccountType.SPOT_CLIENT_INFOS,
      }),
      role: AccountRole.WRITABLE,
    },
  ];

  const assetDec = tokenDec(ctx.tokens, assetTokenId, ctx.uiNumbers);

  const newInstrIx = {
    accounts: keys,
    programAddress: ctx.programId,
    data: newInstrumentData(
      9,
      args.mask,
      crncyTokenId,
      slot,
      args.initialPrice * DF,
      args.minQty * assetDec,
      args.fixedFeeRate,
    ),
  } as Instruction;
  return [createMapsAccountIx, newInstrIx];
}

/**
 * Build swap instruction
 */
async function buildSwapInstruction(
  ctx: SpotInstructionContext,
  args: SwapArgs,
  instr: Instrument,
): Promise<Instruction> {
  const assetTokenId = await getTokenId(ctx, args.assetMint);
  const crncyTokenId = await getTokenId(ctx, args.crncyMint);
  if (assetTokenId == null) {
    throw new Error(`Asset token not found for mint ${args.assetMint}`);
  }
  if (crncyTokenId == null) {
    throw new Error(`Currency token not found for mint ${args.crncyMint}`);
  }
  const assetTokenAccount = ctx.tokens.get(assetTokenId);
  const crncyTokenAccount = ctx.tokens.get(crncyTokenId);
  if (!assetTokenAccount || !crncyTokenAccount) {
    throw new Error('Token account not found');
  }
  const assetTokenProgramId = (assetTokenAccount.mask & 0x80000000) == 0 ? TOKEN_PROGRAM_ID : TOKEN_2022_PROGRAM_ID;
  const crncyTokenProgramId = (crncyTokenAccount.mask & 0x80000000) == 0 ? TOKEN_PROGRAM_ID : TOKEN_2022_PROGRAM_ID;
  let instrId = await getInstrId(ctx, { assetTokenId: assetTokenId, crncyTokenId: crncyTokenId });
  if (instrId === null) {
    throw new Error('No instruction ID');
  }

  const clientAssetTokenAccount = await findAssociatedTokenAddress(ctx.signer, assetTokenProgramId, args.assetMint);
  const clientCrncyTokenAccount = await findAssociatedTokenAddress(ctx.signer, crncyTokenProgramId, args.crncyMint);

  let buf = swapData(
    26,
    args.crncyInput ? 1 : 0,
    instrId,
    Math.round(args.limitPrice * DF),
    Math.round(
      args.amount *
        (args.crncyInput
          ? tokenDec(ctx.tokens, instr.header.crncyTokenId, ctx.uiNumbers)
          : tokenDec(ctx.tokens, instr.header.assetTokenId, ctx.uiNumbers)),
    ),
    args.minAmountOut ?? 0,
  );

  const swapSide = args.crncyInput ? 1 : 0;

  let keys = [
    { address: ctx.signer, role: AccountRole.READONLY_SIGNER },
    { address: args.assetMint, role: AccountRole.READONLY },
    { address: args.crncyMint, role: AccountRole.READONLY },
    { address: assetTokenAccount.programAddress, role: AccountRole.WRITABLE },
    { address: crncyTokenAccount.programAddress, role: AccountRole.WRITABLE },
    ...(await getSpotOneSidedContext(ctx, instr.header, swapSide)),
    { address: clientAssetTokenAccount, role: AccountRole.WRITABLE },
    { address: clientCrncyTokenAccount, role: AccountRole.WRITABLE },
  ];

  keys.push({ address: assetTokenProgramId, role: AccountRole.READONLY });
  if (assetTokenProgramId !== crncyTokenProgramId) {
    keys.push({ address: crncyTokenProgramId, role: AccountRole.READONLY });
  }
  keys.push({ address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY });
  keys.push({ address: ASSOCIATED_TOKEN_PROGRAM_ID, role: AccountRole.READONLY });

  return { accounts: keys, programAddress: ctx.programId, data: buf };
}

/**
 * Build new ref link instruction
 */
async function buildNewRefLinkInstruction(ctx: PerpInstructionContext): Promise<Instruction> {
  const clientPrimaryAccount = requireClientPrimaryAccount(ctx);

  let buf = Buffer.alloc(1);
  buf.writeUInt8(45, 0);

  let keys = [
    { address: ctx.signer, role: AccountRole.READONLY_SIGNER },
    { address: ctx.rootAccount, role: AccountRole.WRITABLE },
    { address: clientPrimaryAccount, role: AccountRole.WRITABLE },
  ];

  return { accounts: keys, programAddress: ctx.programId, data: buf };
}

export {
  buildDepositInstruction,
  buildWithdrawInstruction,
  buildNewInstrumentInstructions,
  buildSwapInstruction,
  buildNewRefLinkInstruction,
};
