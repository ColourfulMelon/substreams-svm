import {
  AccountRole,
  Address,
  Base58EncodedBytes,
  Base64EncodedDataResponse,
  getAddressDecoder,
  getAddressEncoder,
  getBase64Encoder,
  getProgramDerivedAddress,
} from '@solana/kit';
import bs58 from 'bs58';
import { Buffer } from 'buffer';

import {
  GetKaminoClientStateArgs,
  GetKaminoContextArgs,
  Instruction,
  KaminoAtaExistsArgs,
  KaminoChangePositionArgs,
  KaminoClientStateResponse,
  KaminoContext,
  KaminoFarmContext,
  KaminoInitObligationArgs,
  KaminoInitInstrumentArgs,
  KaminoInstrumentAccountsExistArgs,
  KaminoInstrumentAccountsExistResponse,
  KaminoLookupTableAddressesArgs,
  KaminoLookupTableAddressesResponse,
  KaminoObligationExistsArgs,
  KaminoRefreshReservesArgs,
  KaminoOracleAccounts,
  KaminoReserveByMintArgs,
  KaminoReserveContext,
  KaminoReserveInfo,
  KaminoUpdateObligationsArgs,
  VmFinalizeActivateArgs,
} from '../types';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  FARMS_PROGRAM_ID,
  KLEND_PROGRAM_ID,
  MAIN_KAMINO_MARKET,
  MAIN_KAMINO_MARKET_LUT,
  SYSTEM_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
} from '../constants';
import { Instrument } from '../types/responses';
import { TokenStateModel } from '../structure_models';
import { kaminoChangePositionData, kaminoInitInstrumentData, kaminoInitObligationData } from '../instruction_models';
import { findAssociatedTokenAddress, tokenDec } from './utils';
import {
  AccountHelperContext,
  findClientPrimaryAccount,
  findClientVmAccount,
  getProgramTokenAccount,
} from './account-helpers';

export const KAMINO_REPAY_ALL_FLAG = 1;
export const KAMINO_WITHDRAW_ALL_FLAG = 2;
export const KAMINO_REFRESH_OBLIGATION_DISCRIMINATOR = Buffer.from([33, 132, 147, 228, 151, 192, 72, 89]);
export const KAMINO_REFRESH_RESERVES_BATCH_DISCRIMINATOR = Buffer.from([144, 110, 26, 103, 162, 204, 252, 147]);

const NULL_ADDRESS = '11111111111111111111111111111111' as Address;
const SYSVAR_RENT = 'SysvarRent111111111111111111111111111111111' as Address;
const SYSVAR_INSTRUCTIONS = 'Sysvar1nstructions1111111111111111111111111' as Address;

const RESERVE_DISCRIMINATOR = Buffer.from([43, 242, 204, 202, 26, 247, 59, 127]);
const OBLIGATION_DISCRIMINATOR = Buffer.from([168, 206, 141, 106, 88, 76, 172, 167]);
const KAMINO_RESERVE_STATUS_ACTIVE = 0;
const KAMINO_RESERVE_DOMINANCE_RATIO = 100;

const RESERVE_LENDING_MARKET_OFFSET = 32;
const RESERVE_FARM_COLLATERAL_OFFSET = 64;
const RESERVE_FARM_DEBT_OFFSET = 96;
const RESERVE_LIQUIDITY_MINT_OFFSET = 128;
const RESERVE_LIQUIDITY_SUPPLY_OFFSET = 160;
const RESERVE_FEE_VAULT_OFFSET = 192;
const RESERVE_TOTAL_AVAILABLE_AMOUNT_OFFSET = 224;
const RESERVE_BORROWED_AMOUNT_SF_OFFSET = 232;
const RESERVE_MARKET_PRICE_SF_OFFSET = 248;
const RESERVE_MINT_DECIMALS_OFFSET = 272;
const RESERVE_ACCUMULATED_PROTOCOL_FEES_SF_OFFSET = 344;
const RESERVE_ACCUMULATED_REFERRER_FEES_SF_OFFSET = 360;
const RESERVE_PENDING_REFERRER_FEES_SF_OFFSET = 376;
const RESERVE_TOKEN_PROGRAM_OFFSET = 408;
const RESERVE_COLLATERAL_MINT_OFFSET = 2560;
const RESERVE_COLLATERAL_MINT_TOTAL_SUPPLY_OFFSET = 2592;
const RESERVE_COLLATERAL_SUPPLY_OFFSET = 2600;
const RESERVE_CONFIG_OFFSET = 4856;
const RESERVE_STATUS_OFFSET = RESERVE_CONFIG_OFFSET;
const RESERVE_LOAN_TO_VALUE_PCT_OFFSET = RESERVE_CONFIG_OFFSET + 16;
const RESERVE_LIQUIDATION_THRESHOLD_PCT_OFFSET = RESERVE_CONFIG_OFFSET + 17;
const RESERVE_DEPOSIT_LIMIT_OFFSET = RESERVE_CONFIG_OFFSET + 160;
const RESERVE_BORROW_LIMIT_OFFSET = RESERVE_CONFIG_OFFSET + 168;
const RESERVE_SCOPE_PRICE_OFFSET = RESERVE_CONFIG_OFFSET + 176 + 80;
const RESERVE_SWITCHBOARD_PRICE_OFFSET = RESERVE_CONFIG_OFFSET + 176 + 128;
const RESERVE_SWITCHBOARD_TWAP_OFFSET = RESERVE_CONFIG_OFFSET + 176 + 160;
const RESERVE_PYTH_PRICE_OFFSET = RESERVE_CONFIG_OFFSET + 176 + 192;

const OBLIGATION_OWNER_OFFSET = 64;
const OBLIGATION_DEPOSITS_OFFSET = 96;
const OBLIGATION_COLLATERAL_SIZE = 136;
const OBLIGATION_COLLATERAL_DEPOSITED_AMOUNT_OFFSET = 32;
const OBLIGATION_COLLATERAL_MARKET_VALUE_OFFSET = 40;
const OBLIGATION_DEPOSITED_VALUE_SF_OFFSET = 1192;
const OBLIGATION_BORROWS_OFFSET = 1208;
const OBLIGATION_LIQUIDITY_SIZE = 200;
const OBLIGATION_BORROWED_AMOUNT_SF_OFFSET = 88;
const OBLIGATION_BORROW_MARKET_VALUE_SF_OFFSET = 104;
const OBLIGATION_BORROW_FACTOR_ADJUSTED_DEBT_VALUE_SF_OFFSET = 2208;
const OBLIGATION_BORROWED_ASSETS_MARKET_VALUE_SF_OFFSET = 2224;
const OBLIGATION_ALLOWED_BORROW_VALUE_SF_OFFSET = 2240;
const OBLIGATION_UNHEALTHY_BORROW_VALUE_SF_OFFSET = 2256;

const SF_FRACTIONAL_BITS = BigInt(60);
const SF_ONE = BigInt(1) << SF_FRACTIONAL_BITS;

const addressEncoder = getAddressEncoder();
const addressDecoder = getAddressDecoder();

export interface KaminoInstructionContext extends AccountHelperContext {
  instruments: Map<number, Instrument>;
  tokens: Map<number, TokenStateModel>;
  uiNumbers: boolean;
  signer: Address;
  rootAccount: Address;
  clientPrimaryAccount: Address | null;
  clientLutAddress: Address | null;
  clientVmActive: boolean;
  getKaminoReserveInfoByMint?: (args: KaminoReserveByMintArgs) => Promise<KaminoReserveInfo>;
}

function dataToBuffer(data: Base64EncodedDataResponse): Buffer {
  return Buffer.from(getBase64Encoder().encode(data[0]));
}

function readAddress(buffer: Buffer, offset: number): Address {
  return addressDecoder.decode(buffer.subarray(offset, offset + 32)) as Address;
}

function readU64(buffer: Buffer, offset: number): number {
  return Number(buffer.readBigUInt64LE(offset));
}

function readU128(buffer: Buffer, offset: number): bigint {
  const lo = buffer.readBigUInt64LE(offset);
  const hi = buffer.readBigUInt64LE(offset + 8);
  return lo + (hi << BigInt(64));
}

function sfToNumber(value: bigint): number {
  return Number(value) / Number(SF_ONE);
}

function sfToIntegerCeil(value: bigint): number {
  if (value === BigInt(0)) return 0;
  return Number((value + SF_ONE - BigInt(1)) >> SF_FRACTIONAL_BITS);
}

function sfToIntegerFloor(value: bigint): number {
  if (value <= BigInt(0)) return 0;
  return Number(value >> SF_FRACTIONAL_BITS);
}

function isDefaultAddress(value: Address | null | undefined): boolean {
  return value == null || value === NULL_ADDRESS;
}

function oracleOrSentinel(value: Address): Address {
  return isDefaultAddress(value) ? KLEND_PROGRAM_ID : value;
}

function accountOwner(info: { owner?: Address; programAddress?: Address }): Address | undefined {
  return info.owner ?? info.programAddress;
}

function requireClientPrimary(ctx: KaminoInstructionContext): Address {
  if (ctx.clientPrimaryAccount === null) {
    throw new Error('Client primary account not found');
  }
  return ctx.clientPrimaryAccount;
}

function requireInstrument(ctx: KaminoInstructionContext, instrId: number): Instrument {
  const instr = ctx.instruments.get(instrId);
  if (instr === undefined) {
    throw new Error('Instrument not found');
  }
  return instr;
}

function requireTokenMint(ctx: KaminoInstructionContext, tokenId: number, label: string): Address {
  const token = ctx.tokens.get(tokenId);
  if (token === undefined) {
    throw new Error(`${label} token not found`);
  }
  return token.address;
}

function instrumentMints(ctx: KaminoInstructionContext, instr: Instrument): { assetMint: Address; crncyMint: Address } {
  return {
    assetMint: requireTokenMint(ctx, instr.header.assetTokenId, 'Asset'),
    crncyMint: requireTokenMint(ctx, instr.header.crncyTokenId, 'Currency'),
  };
}

function reserveMemcmp(
  offset: number,
  bytes: Buffer,
): {
  memcmp: { offset: bigint; encoding: 'base58'; bytes: Base58EncodedBytes };
} {
  return {
    memcmp: {
      offset: BigInt(offset),
      encoding: 'base58',
      bytes: bs58.encode(bytes) as Base58EncodedBytes,
    },
  };
}

function addressMemcmp(offset: number, value: Address) {
  return reserveMemcmp(offset, Buffer.from(addressEncoder.encode(value)));
}

export async function lendingMarketAuthPda(lendingMarket: Address): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: KLEND_PROGRAM_ID,
      seeds: [Buffer.from('lma'), addressEncoder.encode(lendingMarket)],
    })
  )[0];
}

export async function reserveLiqSupplyPda(reserve: Address): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: KLEND_PROGRAM_ID,
      seeds: [Buffer.from('reserve_liq_supply'), addressEncoder.encode(reserve)],
    })
  )[0];
}

export async function reserveCollateralMintPda(reserve: Address): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: KLEND_PROGRAM_ID,
      seeds: [Buffer.from('reserve_coll_mint'), addressEncoder.encode(reserve)],
    })
  )[0];
}

export async function reserveCollateralSupplyPda(reserve: Address): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: KLEND_PROGRAM_ID,
      seeds: [Buffer.from('reserve_coll_supply'), addressEncoder.encode(reserve)],
    })
  )[0];
}

export async function reserveFeeVaultPda(reserve: Address): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: KLEND_PROGRAM_ID,
      seeds: [Buffer.from('fee_receiver'), addressEncoder.encode(reserve)],
    })
  )[0];
}

export async function userMetadataPda(owner: Address): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: KLEND_PROGRAM_ID,
      seeds: [Buffer.from('user_meta'), addressEncoder.encode(owner)],
    })
  )[0];
}

export async function vanillaObligationPda(args: { owner: Address; lendingMarket: Address }): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: KLEND_PROGRAM_ID,
      seeds: [
        Buffer.from([0]),
        Buffer.from([0]),
        addressEncoder.encode(args.owner),
        addressEncoder.encode(args.lendingMarket),
        addressEncoder.encode(NULL_ADDRESS),
        addressEncoder.encode(NULL_ADDRESS),
      ],
    })
  )[0];
}

export async function obligationFarmStatePda(args: { farmState: Address; obligation: Address }): Promise<Address> {
  return (
    await getProgramDerivedAddress({
      programAddress: FARMS_PROGRAM_ID,
      seeds: [Buffer.from('user'), addressEncoder.encode(args.farmState), addressEncoder.encode(args.obligation)],
    })
  )[0];
}

export async function clientPrimaryAta(args: {
  clientPrimaryAccount: Address;
  mint: Address;
  tokenProgram: Address;
}): Promise<Address> {
  return findAssociatedTokenAddress(args.clientPrimaryAccount, args.tokenProgram, args.mint);
}

function decodeReserve(address: Address, buffer: Buffer): KaminoReserveInfo {
  if (buffer.length < RESERVE_CONFIG_OFFSET + 936) {
    throw new Error(`Invalid Kamino reserve layout: ${address}`);
  }
  if (!buffer.subarray(0, 8).equals(RESERVE_DISCRIMINATOR)) {
    throw new Error(`Invalid Kamino reserve discriminator: ${address}`);
  }

  const lendingMarket = readAddress(buffer, RESERVE_LENDING_MARKET_OFFSET);
  const tokenProgram = readAddress(buffer, RESERVE_TOKEN_PROGRAM_OFFSET);
  const farmCollateral = readAddress(buffer, RESERVE_FARM_COLLATERAL_OFFSET);
  const farmDebt = readAddress(buffer, RESERVE_FARM_DEBT_OFFSET);
  const totalAvailableAmount = readU64(buffer, RESERVE_TOTAL_AVAILABLE_AMOUNT_OFFSET);
  const borrowedAmountSf = readU128(buffer, RESERVE_BORROWED_AMOUNT_SF_OFFSET);
  const accumulatedProtocolFeesSf = readU128(buffer, RESERVE_ACCUMULATED_PROTOCOL_FEES_SF_OFFSET);
  const accumulatedReferrerFeesSf = readU128(buffer, RESERVE_ACCUMULATED_REFERRER_FEES_SF_OFFSET);
  const pendingReferrerFeesSf = readU128(buffer, RESERVE_PENDING_REFERRER_FEES_SF_OFFSET);
  const collateralMintTotalSupply = readU64(buffer, RESERVE_COLLATERAL_MINT_TOTAL_SUPPLY_OFFSET);
  const totalLiquiditySf =
    BigInt(totalAvailableAmount) * SF_ONE +
    borrowedAmountSf -
    accumulatedProtocolFeesSf -
    accumulatedReferrerFeesSf -
    pendingReferrerFeesSf;
  const oracles: KaminoOracleAccounts = {
    pyth: oracleOrSentinel(readAddress(buffer, RESERVE_PYTH_PRICE_OFFSET)),
    switchboardPrice: oracleOrSentinel(readAddress(buffer, RESERVE_SWITCHBOARD_PRICE_OFFSET)),
    switchboardTwap: oracleOrSentinel(readAddress(buffer, RESERVE_SWITCHBOARD_TWAP_OFFSET)),
    scope: oracleOrSentinel(readAddress(buffer, RESERVE_SCOPE_PRICE_OFFSET)),
  };

  return {
    address,
    lendingMarket,
    liquidityMint: readAddress(buffer, RESERVE_LIQUIDITY_MINT_OFFSET),
    liquiditySupply: readAddress(buffer, RESERVE_LIQUIDITY_SUPPLY_OFFSET),
    collateralMint: readAddress(buffer, RESERVE_COLLATERAL_MINT_OFFSET),
    collateralSupply: readAddress(buffer, RESERVE_COLLATERAL_SUPPLY_OFFSET),
    feeVault: readAddress(buffer, RESERVE_FEE_VAULT_OFFSET),
    tokenProgram: isDefaultAddress(tokenProgram) ? TOKEN_PROGRAM_ID : tokenProgram,
    farmCollateral,
    farmDebt,
    oracles,
    loanToValuePct: buffer.readUint8(RESERVE_LOAN_TO_VALUE_PCT_OFFSET),
    liquidationThresholdPct: buffer.readUint8(RESERVE_LIQUIDATION_THRESHOLD_PCT_OFFSET),
    mintDecimals: readU64(buffer, RESERVE_MINT_DECIMALS_OFFSET),
    raw: {
      status: buffer.readUint8(RESERVE_STATUS_OFFSET),
      marketPriceSf: sfToNumber(readU128(buffer, RESERVE_MARKET_PRICE_SF_OFFSET)),
      totalAvailableAmount,
      borrowedAmountSf: sfToNumber(borrowedAmountSf),
      borrowedAmountSfRaw: borrowedAmountSf,
      accumulatedProtocolFeesSf: sfToNumber(accumulatedProtocolFeesSf),
      accumulatedProtocolFeesSfRaw: accumulatedProtocolFeesSf,
      accumulatedReferrerFeesSf: sfToNumber(accumulatedReferrerFeesSf),
      accumulatedReferrerFeesSfRaw: accumulatedReferrerFeesSf,
      pendingReferrerFeesSf: sfToNumber(pendingReferrerFeesSf),
      pendingReferrerFeesSfRaw: pendingReferrerFeesSf,
      collateralMintTotalSupply,
      totalLiquidity: sfToNumber(totalLiquiditySf),
      totalLiquiditySfRaw: totalLiquiditySf,
      borrowLimit: readU64(buffer, RESERVE_BORROW_LIMIT_OFFSET),
      depositLimit: readU64(buffer, RESERVE_DEPOSIT_LIMIT_OFFSET),
    },
  };
}

export function decodeKaminoReserveData(address: Address, data: Base64EncodedDataResponse): KaminoReserveInfo {
  return decodeReserve(address, dataToBuffer(data));
}

export async function loadKaminoReserve(
  ctx: Pick<KaminoInstructionContext, 'rpc' | 'commitment'>,
  reserve: Address,
): Promise<KaminoReserveInfo> {
  const info = await ctx.rpc.getAccountInfo(reserve, { commitment: ctx.commitment, encoding: 'base64' }).send();
  if (info.value == null) {
    throw new Error(`Kamino reserve not found: ${reserve}`);
  }
  if (accountOwner(info.value) !== KLEND_PROGRAM_ID) {
    throw new Error(`Kamino reserve is not owned by KLend: ${reserve}`);
  }
  return decodeReserve(reserve, dataToBuffer(info.value.data));
}

export async function refreshKaminoContextReserveData(
  ctx: KaminoInstructionContext,
  context: KaminoContext,
  args: {
    collateralReserveData: Base64EncodedDataResponse;
    debtReserveData: Base64EncodedDataResponse;
  },
): Promise<KaminoContext> {
  const instr = requireInstrument(ctx, context.instrId);
  const { assetMint, crncyMint } = instrumentMints(ctx, instr);
  const collateralReserveInfo = decodeKaminoReserveData(context.collateralReserve.address, args.collateralReserveData);
  validateKaminoReserveForMint(collateralReserveInfo, assetMint, context.lendingMarket, 'Collateral');
  const debtReserveInfo = decodeKaminoReserveData(context.debtReserve.address, args.debtReserveData);
  validateKaminoReserveForMint(debtReserveInfo, crncyMint, context.lendingMarket, 'Debt');

  const [collateralReserve, debtReserve] = await Promise.all([
    reserveContext({
      ctx,
      reserve: collateralReserveInfo,
      obligation: context.obligation,
      clientPrimaryAccount: context.clientPrimaryAccount,
      side: 'collateral',
    }),
    reserveContext({
      ctx,
      reserve: debtReserveInfo,
      obligation: context.obligation,
      clientPrimaryAccount: context.clientPrimaryAccount,
      side: 'debt',
    }),
  ]);

  return {
    ...context,
    collateralReserve,
    debtReserve,
    extraReserves: [],
  };
}

export function validateKaminoReserveForMint(
  reserve: KaminoReserveInfo,
  expectedMint: Address,
  lendingMarket: Address,
  label: string,
): void {
  if (reserve.lendingMarket !== lendingMarket) {
    throw new Error(`${label} reserve lending market mismatch`);
  }
  if (reserve.liquidityMint !== expectedMint) {
    throw new Error(`${label} reserve liquidity mint does not match instrument mint`);
  }
}

function kaminoReserveSelectionScore(reserve: KaminoReserveInfo): number {
  return Math.max(reserve.raw.totalLiquidity, reserve.raw.depositLimit, reserve.raw.borrowLimit);
}

function selectDominantKaminoReserve(reserves: KaminoReserveInfo[]): KaminoReserveInfo | null {
  const [first, second] = [...reserves].sort((a, b) => kaminoReserveSelectionScore(b) - kaminoReserveSelectionScore(a));
  if (first == null) {
    return null;
  }

  const firstScore = kaminoReserveSelectionScore(first);
  const secondScore = second == null ? 0 : kaminoReserveSelectionScore(second);
  if (firstScore > 0 && (secondScore === 0 || firstScore >= secondScore * KAMINO_RESERVE_DOMINANCE_RATIO)) {
    return first;
  }

  return null;
}

function selectSingleActiveKaminoReserve(reserves: KaminoReserveInfo[]): KaminoReserveInfo | null {
  const activeReserves = reserves.filter((reserve) => reserve.raw.status === KAMINO_RESERVE_STATUS_ACTIVE);
  return activeReserves.length === 1 ? activeReserves[0] : null;
}

export async function findKaminoReserveByMint(
  ctx: KaminoInstructionContext,
  args: KaminoReserveByMintArgs,
): Promise<KaminoReserveInfo> {
  const lendingMarket = args.lendingMarket ?? MAIN_KAMINO_MARKET;
  const accounts = await ctx.rpc
    .getProgramAccounts(KLEND_PROGRAM_ID, {
      encoding: 'base64',
      filters: [
        reserveMemcmp(0, RESERVE_DISCRIMINATOR),
        addressMemcmp(RESERVE_LENDING_MARKET_OFFSET, lendingMarket),
        addressMemcmp(RESERVE_LIQUIDITY_MINT_OFFSET, args.mint),
      ],
    })
    .send();

  if (accounts.length === 0) {
    throw new Error(`Kamino reserve not found for mint ${args.mint}`);
  }

  const reserves = accounts.map((account) => decodeReserve(account.pubkey, dataToBuffer(account.account.data)));
  if (reserves.length === 1) {
    return reserves[0];
  }

  const activeReserve = selectSingleActiveKaminoReserve(reserves);
  if (activeReserve !== null) {
    return activeReserve;
  }

  const enabledReserves = reserves.filter((reserve) => reserve.raw.depositLimit > 0 || reserve.raw.borrowLimit > 0);
  if (enabledReserves.length === 1) {
    return enabledReserves[0];
  }

  const candidates = enabledReserves.length > 0 ? enabledReserves : reserves;
  const dominantReserve = selectDominantKaminoReserve(candidates);
  if (dominantReserve !== null) {
    return dominantReserve;
  }

  throw new Error(
    `Multiple Kamino reserves found for mint ${args.mint}: ${candidates.map((reserve) => reserve.address).join(', ')}`,
  );
}

async function resolveKaminoReserveByMint(
  ctx: KaminoInstructionContext,
  args: KaminoReserveByMintArgs,
): Promise<KaminoReserveInfo> {
  return ctx.getKaminoReserveInfoByMint != null
    ? ctx.getKaminoReserveInfoByMint(args)
    : findKaminoReserveByMint(ctx, args);
}

async function farmContext(args: {
  reserve: KaminoReserveInfo;
  obligation: Address;
  side: 'collateral' | 'liquidity';
}): Promise<KaminoFarmContext> {
  const reserveFarmState = args.side === 'collateral' ? args.reserve.farmCollateral : args.reserve.farmDebt;
  if (isDefaultAddress(reserveFarmState)) {
    return {
      obligationFarm: KLEND_PROGRAM_ID,
      reserveFarmState: KLEND_PROGRAM_ID,
      hasFarm: false,
    };
  }
  return {
    obligationFarm: await obligationFarmStatePda({ farmState: reserveFarmState, obligation: args.obligation }),
    reserveFarmState,
    hasFarm: true,
  };
}

async function reserveContext(args: {
  ctx: KaminoInstructionContext;
  reserve: KaminoReserveInfo;
  obligation: Address;
  clientPrimaryAccount: Address;
  side: 'collateral' | 'debt';
}): Promise<KaminoReserveContext> {
  const [vault, clientAta, collateralFarm, liquidityFarm] = await Promise.all([
    getProgramTokenAccount(args.ctx, args.reserve.liquidityMint),
    clientPrimaryAta({
      clientPrimaryAccount: args.clientPrimaryAccount,
      mint: args.reserve.liquidityMint,
      tokenProgram: args.reserve.tokenProgram,
    }),
    farmContext({
      reserve: args.reserve,
      obligation: args.obligation,
      side: 'collateral',
    }),
    farmContext({
      reserve: args.reserve,
      obligation: args.obligation,
      side: 'liquidity',
    }),
  ]);
  const selectedFarm = args.side === 'collateral' ? collateralFarm : liquidityFarm;

  return {
    ...args.reserve,
    vault,
    clientAta,
    collateralFarm,
    liquidityFarm,
    obligationFarm: selectedFarm.obligationFarm,
    reserveFarmState: selectedFarm.reserveFarmState,
    hasFarm: selectedFarm.hasFarm,
  };
}

function reserveMetaRole(hasFarm: boolean): AccountRole {
  return hasFarm ? AccountRole.WRITABLE : AccountRole.READONLY;
}

function appendOptionalVmAccount(accounts: { address: Address; role: AccountRole }[], ctx: KaminoContext): void {
  if (ctx.clientVmAccount !== null) {
    accounts.push({ address: ctx.clientVmAccount, role: AccountRole.READONLY });
  }
}

export async function buildKaminoContext(
  ctx: KaminoInstructionContext,
  args: GetKaminoContextArgs,
): Promise<KaminoContext> {
  const lendingMarket = args.lendingMarket ?? MAIN_KAMINO_MARKET;
  const instr = requireInstrument(ctx, args.instrId);
  const { assetMint, crncyMint } = instrumentMints(ctx, instr);
  const clientPrimaryAccount = requireClientPrimary(ctx);
  const clientVmAccount = ctx.clientVmActive ? await findClientVmAccount(ctx, ctx.signer) : null;
  const obligation = await vanillaObligationPda({ owner: clientPrimaryAccount, lendingMarket });
  const userMetadata = await userMetadataPda(clientPrimaryAccount);

  const collateralReserveInfo = await resolveKaminoReserveByMint(ctx, { mint: assetMint, lendingMarket });
  validateKaminoReserveForMint(collateralReserveInfo, assetMint, lendingMarket, 'Collateral');

  const debtReserveInfo = await resolveKaminoReserveByMint(ctx, { mint: crncyMint, lendingMarket });
  validateKaminoReserveForMint(debtReserveInfo, crncyMint, lendingMarket, 'Debt');

  const [lendingMarketAuthority, collateralReserve, debtReserve] = await Promise.all([
    lendingMarketAuthPda(lendingMarket),
    reserveContext({
      ctx,
      reserve: collateralReserveInfo,
      obligation,
      clientPrimaryAccount,
      side: 'collateral',
    }),
    reserveContext({
      ctx,
      reserve: debtReserveInfo,
      obligation,
      clientPrimaryAccount,
      side: 'debt',
    }),
  ]);

  return {
    instrId: args.instrId,
    lendingMarket,
    lendingMarketAuthority,
    instrAccount: instr.address,
    clientPrimaryAccount,
    clientVmAccount,
    userMetadata,
    obligation,
    collateralReserve,
    debtReserve,
    extraReserves: [],
  };
}

export async function buildVmAddKaminoInstruction(
  ctx: KaminoInstructionContext,
  args: VmFinalizeActivateArgs,
): Promise<Instruction> {
  const clientPrimaryAccount = await findClientPrimaryAccount(ctx, ctx.signer);
  const clientVmAccount = await findClientVmAccount(ctx, ctx.signer);
  return {
    accounts: [
      { address: args.vmAuthority, role: AccountRole.WRITABLE_SIGNER },
      { address: ctx.rootAccount, role: AccountRole.READONLY },
      { address: clientPrimaryAccount, role: AccountRole.WRITABLE },
      { address: clientVmAccount, role: AccountRole.WRITABLE },
      { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
    ],
    programAddress: ctx.programId,
    data: Buffer.from([81]),
  };
}

export async function buildVmRemoveKaminoInstruction(
  ctx: KaminoInstructionContext,
  args: VmFinalizeActivateArgs,
): Promise<Instruction> {
  const clientPrimaryAccount = await findClientPrimaryAccount(ctx, ctx.signer);
  const clientVmAccount = await findClientVmAccount(ctx, ctx.signer);
  return {
    accounts: [
      { address: args.vmAuthority, role: AccountRole.READONLY_SIGNER },
      { address: ctx.rootAccount, role: AccountRole.READONLY },
      { address: clientPrimaryAccount, role: AccountRole.WRITABLE },
      { address: clientVmAccount, role: AccountRole.WRITABLE },
    ],
    programAddress: ctx.programId,
    data: Buffer.from([82]),
  };
}

function appendInitInstrumentFarmAccounts(
  accounts: { address: Address; role: AccountRole }[],
  farm: KaminoFarmContext,
): boolean {
  if (!farm.hasFarm) {
    return false;
  }
  accounts.push(
    { address: farm.reserveFarmState, role: AccountRole.WRITABLE },
    { address: farm.obligationFarm, role: AccountRole.WRITABLE },
  );
  return true;
}

export async function buildKaminoInitInstrumentInstruction(
  ctx: KaminoInstructionContext,
  args: KaminoInitInstrumentArgs,
  kaminoCtx: KaminoContext,
): Promise<Instruction> {
  const instr = requireInstrument(ctx, args.instrId);
  const { assetMint, crncyMint } = instrumentMints(ctx, instr);
  validateKaminoReserveForMint(kaminoCtx.collateralReserve, assetMint, kaminoCtx.lendingMarket, 'Collateral');
  validateKaminoReserveForMint(kaminoCtx.debtReserve, crncyMint, kaminoCtx.lendingMarket, 'Debt');

  const accounts = [
    { address: ctx.signer, role: AccountRole.WRITABLE_SIGNER },
    { address: ctx.rootAccount, role: AccountRole.READONLY },
    { address: kaminoCtx.instrAccount, role: AccountRole.READONLY },
    { address: kaminoCtx.clientPrimaryAccount, role: AccountRole.READONLY },
  ];
  appendOptionalVmAccount(accounts, kaminoCtx);
  accounts.push(
    { address: kaminoCtx.obligation, role: AccountRole.WRITABLE },
    { address: kaminoCtx.lendingMarket, role: AccountRole.READONLY },
    { address: kaminoCtx.lendingMarketAuthority, role: AccountRole.READONLY },
    { address: assetMint, role: AccountRole.READONLY },
    { address: kaminoCtx.collateralReserve.clientAta, role: AccountRole.WRITABLE },
    { address: kaminoCtx.collateralReserve.tokenProgram, role: AccountRole.READONLY },
    { address: kaminoCtx.collateralReserve.address, role: AccountRole.WRITABLE },
  );
  const hasAssetCollateralFarm = appendInitInstrumentFarmAccounts(accounts, kaminoCtx.collateralReserve.collateralFarm);
  const hasAssetLiquidityFarm = appendInitInstrumentFarmAccounts(accounts, kaminoCtx.collateralReserve.liquidityFarm);
  accounts.push(
    { address: crncyMint, role: AccountRole.READONLY },
    { address: kaminoCtx.debtReserve.clientAta, role: AccountRole.WRITABLE },
    { address: kaminoCtx.debtReserve.tokenProgram, role: AccountRole.READONLY },
    { address: kaminoCtx.debtReserve.address, role: AccountRole.WRITABLE },
  );
  const hasCrncyCollateralFarm = appendInitInstrumentFarmAccounts(accounts, kaminoCtx.debtReserve.collateralFarm);
  const hasCrncyLiquidityFarm = appendInitInstrumentFarmAccounts(accounts, kaminoCtx.debtReserve.liquidityFarm);
  accounts.push(
    { address: ASSOCIATED_TOKEN_PROGRAM_ID, role: AccountRole.READONLY },
    { address: KLEND_PROGRAM_ID, role: AccountRole.READONLY },
    { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
  );
  if (hasAssetCollateralFarm || hasAssetLiquidityFarm || hasCrncyCollateralFarm || hasCrncyLiquidityFarm) {
    accounts.push(
      { address: FARMS_PROGRAM_ID, role: AccountRole.READONLY },
      { address: SYSVAR_RENT, role: AccountRole.READONLY },
    );
  }
  return {
    accounts,
    programAddress: ctx.programId,
    data: kaminoInitInstrumentData(84, args.instrId),
  };
}

export async function buildKaminoInitObligationInstruction(
  ctx: KaminoInstructionContext,
  args: KaminoInitObligationArgs = {},
): Promise<Instruction> {
  const clientPrimaryAccount = requireClientPrimary(ctx);
  const clientVmAccount = ctx.clientVmActive ? await findClientVmAccount(ctx, ctx.signer) : null;
  const lendingMarket = args.lendingMarket ?? MAIN_KAMINO_MARKET;
  const [userMetadata, obligation] = await Promise.all([
    userMetadataPda(clientPrimaryAccount),
    vanillaObligationPda({ owner: clientPrimaryAccount, lendingMarket }),
  ]);
  const accounts = [
    { address: ctx.signer, role: AccountRole.WRITABLE_SIGNER },
    { address: ctx.rootAccount, role: AccountRole.READONLY },
    { address: clientPrimaryAccount, role: AccountRole.READONLY },
  ];
  if (clientVmAccount !== null) {
    accounts.push({ address: clientVmAccount, role: AccountRole.READONLY });
  }
  accounts.push(
    { address: userMetadata, role: AccountRole.WRITABLE },
    { address: obligation, role: AccountRole.WRITABLE },
    { address: lendingMarket, role: AccountRole.READONLY },
    { address: KLEND_PROGRAM_ID, role: AccountRole.READONLY },
    { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
    { address: SYSVAR_RENT, role: AccountRole.READONLY },
  );
  return {
    accounts,
    programAddress: ctx.programId,
    data: kaminoInitObligationData(83),
  };
}

function requireValidObligationBuffer(
  obligation: Address,
  info: { owner?: Address; programAddress?: Address; data: Base64EncodedDataResponse },
): Buffer {
  const buffer = dataToBuffer(info.data);
  if (
    accountOwner(info) !== KLEND_PROGRAM_ID ||
    buffer.length < OBLIGATION_UNHEALTHY_BORROW_VALUE_SF_OFFSET + 16 ||
    !buffer.subarray(0, 8).equals(OBLIGATION_DISCRIMINATOR)
  ) {
    throw new Error(`Invalid Kamino obligation layout: ${obligation}`);
  }
  return buffer;
}

function kaminoObligationReserveAccounts(buffer: Buffer): Address[] {
  const reserves: Address[] = [];
  for (let i = 0; i < 8; i++) {
    addUniqueAddress(reserves, readAddress(buffer, OBLIGATION_DEPOSITS_OFFSET + i * OBLIGATION_COLLATERAL_SIZE));
  }
  for (let i = 0; i < 5; i++) {
    addUniqueAddress(reserves, readAddress(buffer, OBLIGATION_BORROWS_OFFSET + i * OBLIGATION_LIQUIDITY_SIZE));
  }
  return reserves;
}

async function loadKaminoReserveForRefresh(
  ctx: KaminoInstructionContext,
  reserve: Address,
): Promise<KaminoReserveInfo> {
  const info = await ctx.rpc.getAccountInfo(reserve, { commitment: ctx.commitment, encoding: 'base64' }).send();
  if (info.value == null || accountOwner(info.value) !== KLEND_PROGRAM_ID) {
    throw new Error(`Kamino reserve not found: ${reserve}`);
  }
  return decodeKaminoReserveData(reserve, info.value.data);
}

export async function buildKaminoRefreshReservesInstruction(
  ctx: KaminoInstructionContext,
  args: KaminoRefreshReservesArgs = {},
): Promise<Instruction> {
  const clientPrimaryAccount = requireClientPrimary(ctx);
  const lendingMarket = args.lendingMarket ?? MAIN_KAMINO_MARKET;
  const obligation = await vanillaObligationPda({ owner: clientPrimaryAccount, lendingMarket });
  const info = await ctx.rpc.getAccountInfo(obligation, { commitment: ctx.commitment, encoding: 'base64' }).send();
  if (info.value == null) {
    throw new Error(`Kamino obligation not found: ${obligation}`);
  }
  const buffer = requireValidObligationBuffer(obligation, info.value);
  const reserves = await Promise.all(
    kaminoObligationReserveAccounts(buffer).map((reserve) => loadKaminoReserveForRefresh(ctx, reserve)),
  );
  const accounts = reserves.flatMap((reserve) => [
    { address: reserve.address, role: AccountRole.WRITABLE },
    { address: reserve.lendingMarket, role: AccountRole.READONLY },
    { address: reserve.oracles.pyth, role: AccountRole.READONLY },
    { address: reserve.oracles.switchboardPrice, role: AccountRole.READONLY },
    { address: reserve.oracles.switchboardTwap, role: AccountRole.READONLY },
    { address: reserve.oracles.scope, role: AccountRole.READONLY },
  ]);
  return {
    accounts,
    programAddress: KLEND_PROGRAM_ID,
    data: Buffer.concat([
      KAMINO_REFRESH_RESERVES_BATCH_DISCRIMINATOR,
      Buffer.from([args.skipPriceUpdates === true ? 1 : 0]),
    ]),
  };
}

export async function buildKaminoUpdateObligationsInstruction(
  ctx: KaminoInstructionContext,
  args: KaminoUpdateObligationsArgs = {},
): Promise<Instruction> {
  const clientPrimaryAccount = requireClientPrimary(ctx);
  const lendingMarket = args.lendingMarket ?? MAIN_KAMINO_MARKET;
  const obligation = await vanillaObligationPda({ owner: clientPrimaryAccount, lendingMarket });
  const accounts = [
    { address: lendingMarket, role: AccountRole.READONLY },
    { address: obligation, role: AccountRole.WRITABLE },
  ];
  const info = await ctx.rpc.getAccountInfo(obligation, { commitment: ctx.commitment, encoding: 'base64' }).send();
  if (info.value != null) {
    const buffer = requireValidObligationBuffer(obligation, info.value);
    for (const reserve of kaminoObligationReserveAccounts(buffer)) {
      accounts.push({ address: reserve, role: AccountRole.WRITABLE });
    }
  }
  return {
    accounts,
    programAddress: KLEND_PROGRAM_ID,
    data: KAMINO_REFRESH_OBLIGATION_DISCRIMINATOR,
  };
}

function kaminoChangeFlags(args: KaminoChangePositionArgs): number {
  let flags = 0;
  if (args.repayAll) flags |= KAMINO_REPAY_ALL_FLAG;
  if (args.withdrawAll) flags |= KAMINO_WITHDRAW_ALL_FLAG;
  return flags;
}

function validateChangeFlags(args: KaminoChangePositionArgs): void {
  if (args.repayAll && args.borrowDelta !== 0) {
    throw new Error('repayAll requires borrowDelta to be 0');
  }
  if (args.withdrawAll && args.collateralDelta !== 0) {
    throw new Error('withdrawAll requires collateralDelta to be 0');
  }
}

function appendOracleAccounts(accounts: { address: Address; role: AccountRole }[], reserve: KaminoReserveInfo): void {
  accounts.push(
    { address: reserve.oracles.pyth, role: AccountRole.READONLY },
    { address: reserve.oracles.switchboardPrice, role: AccountRole.READONLY },
    { address: reserve.oracles.switchboardTwap, role: AccountRole.READONLY },
    { address: reserve.oracles.scope, role: AccountRole.READONLY },
  );
}

export async function buildKaminoChangePositionInstruction(
  ctx: KaminoInstructionContext,
  args: KaminoChangePositionArgs,
  kaminoCtx: KaminoContext,
): Promise<Instruction> {
  validateChangeFlags(args);
  const instr = requireInstrument(ctx, args.instrId);
  const collateralReserve = args.assetIsCollateral ? kaminoCtx.collateralReserve : kaminoCtx.debtReserve;
  const debtReserve = args.assetIsCollateral ? kaminoCtx.debtReserve : kaminoCtx.collateralReserve;
  const collateralTokenId = args.assetIsCollateral ? instr.header.assetTokenId : instr.header.crncyTokenId;
  const debtTokenId = args.assetIsCollateral ? instr.header.crncyTokenId : instr.header.assetTokenId;
  const collateralDelta = args.collateralDelta * tokenDec(ctx.tokens, collateralTokenId, ctx.uiNumbers);
  const borrowDelta = args.borrowDelta * tokenDec(ctx.tokens, debtTokenId, ctx.uiNumbers);

  const accounts = [
    { address: ctx.signer, role: AccountRole.WRITABLE_SIGNER },
    { address: ctx.rootAccount, role: AccountRole.READONLY },
    { address: kaminoCtx.clientPrimaryAccount, role: AccountRole.WRITABLE },
    { address: SYSTEM_PROGRAM_ID, role: AccountRole.READONLY },
  ];
  appendOptionalVmAccount(accounts, kaminoCtx);
  accounts.push(
    { address: kaminoCtx.instrAccount, role: AccountRole.WRITABLE },
    { address: kaminoCtx.obligation, role: AccountRole.WRITABLE },
    { address: kaminoCtx.userMetadata, role: AccountRole.READONLY },
    { address: SYSVAR_RENT, role: AccountRole.READONLY },
    { address: kaminoCtx.lendingMarket, role: AccountRole.READONLY },
    { address: kaminoCtx.lendingMarketAuthority, role: AccountRole.READONLY },
    { address: collateralReserve.address, role: AccountRole.WRITABLE },
    { address: collateralReserve.liquidityMint, role: AccountRole.READONLY },
    { address: collateralReserve.liquiditySupply, role: AccountRole.WRITABLE },
    { address: collateralReserve.collateralMint, role: AccountRole.WRITABLE },
    { address: collateralReserve.collateralSupply, role: AccountRole.WRITABLE },
    { address: collateralReserve.clientAta, role: AccountRole.WRITABLE },
    { address: collateralReserve.vault, role: AccountRole.WRITABLE },
    { address: collateralReserve.tokenProgram, role: AccountRole.READONLY },
    { address: collateralReserve.tokenProgram, role: AccountRole.READONLY },
    { address: debtReserve.address, role: AccountRole.WRITABLE },
    { address: debtReserve.liquidityMint, role: AccountRole.READONLY },
    { address: debtReserve.liquiditySupply, role: AccountRole.WRITABLE },
    { address: debtReserve.feeVault, role: AccountRole.WRITABLE },
    { address: debtReserve.clientAta, role: AccountRole.WRITABLE },
    { address: debtReserve.vault, role: AccountRole.WRITABLE },
    { address: debtReserve.tokenProgram, role: AccountRole.READONLY },
  );
  appendOracleAccounts(accounts, collateralReserve);
  appendOracleAccounts(accounts, debtReserve);
  accounts.push(
    {
      address: collateralReserve.collateralFarm.obligationFarm,
      role: reserveMetaRole(collateralReserve.collateralFarm.hasFarm),
    },
    {
      address: collateralReserve.collateralFarm.reserveFarmState,
      role: reserveMetaRole(collateralReserve.collateralFarm.hasFarm),
    },
    {
      address: debtReserve.liquidityFarm.obligationFarm,
      role: reserveMetaRole(debtReserve.liquidityFarm.hasFarm),
    },
    {
      address: debtReserve.liquidityFarm.reserveFarmState,
      role: reserveMetaRole(debtReserve.liquidityFarm.hasFarm),
    },
    { address: FARMS_PROGRAM_ID, role: AccountRole.READONLY },
    { address: KLEND_PROGRAM_ID, role: AccountRole.READONLY },
    { address: SYSVAR_INSTRUCTIONS, role: AccountRole.READONLY },
  );
  for (const reserve of kaminoCtx.extraReserves) {
    accounts.push({ address: reserve.address, role: AccountRole.WRITABLE });
    appendOracleAccounts(accounts, reserve);
  }

  return {
    accounts,
    programAddress: ctx.programId,
    data: kaminoChangePositionData(
      85,
      kaminoChangeFlags(args),
      args.instrId,
      borrowDelta,
      collateralDelta,
      args.customId ?? 0,
    ),
  };
}

export function kaminoMarketLut(lendingMarket?: Address): Address | null {
  return (lendingMarket ?? MAIN_KAMINO_MARKET) === MAIN_KAMINO_MARKET ? MAIN_KAMINO_MARKET_LUT : null;
}

function addUniqueAddress(out: Address[], value: Address | null | undefined): void {
  if (value == null || isDefaultAddress(value) || out.includes(value)) {
    return;
  }
  out.push(value);
}

function instrumentLut(instr: Instrument): Address | null {
  const lut = instr.header.lutAddress as Address | undefined;
  return lut == null || isDefaultAddress(lut) ? null : lut;
}

export async function kaminoLookupTableAddresses(
  ctx: KaminoInstructionContext,
  args: KaminoLookupTableAddressesArgs,
  _kaminoCtx: KaminoContext,
): Promise<KaminoLookupTableAddressesResponse> {
  void _kaminoCtx;
  const instr = requireInstrument(ctx, args.instrId);
  const marketLut = kaminoMarketLut(args.lendingMarket);
  const clientLut = ctx.clientLutAddress;
  const instrumentLutAddress = instrumentLut(instr);
  const userLookupTable = null;
  const all: Address[] = [];
  addUniqueAddress(all, marketLut);
  addUniqueAddress(all, clientLut);
  addUniqueAddress(all, instrumentLutAddress);
  return {
    marketLut,
    clientLut,
    instrumentLut: instrumentLutAddress,
    userLookupTable,
    all,
  };
}

export async function kaminoObligationExists(
  ctx: KaminoInstructionContext,
  args: KaminoObligationExistsArgs,
): Promise<boolean> {
  const obligation =
    args.obligation ??
    (await vanillaObligationPda({
      owner: requireClientPrimary(ctx),
      lendingMarket: args.lendingMarket ?? MAIN_KAMINO_MARKET,
    }));
  const info = await ctx.rpc
    .getAccountInfo(obligation, {
      commitment: ctx.commitment,
      encoding: 'base64',
      dataSlice: { offset: 0, length: 8 },
    })
    .send();
  if (info.value == null || accountOwner(info.value) !== KLEND_PROGRAM_ID) {
    return false;
  }
  const buffer = dataToBuffer(info.value.data);
  return buffer.length >= 8 && buffer.subarray(0, 8).equals(OBLIGATION_DISCRIMINATOR);
}

function registeredTokenProgram(ctx: KaminoInstructionContext, mint: Address): Address | null {
  for (const token of ctx.tokens.values()) {
    if (token.address === mint) {
      return (token.mask & 0x80000000) !== 0 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID;
    }
  }
  return null;
}

async function resolveTokenProgram(
  ctx: KaminoInstructionContext,
  mint: Address,
  explicit?: Address,
): Promise<Address | null> {
  if (explicit != null) {
    return explicit;
  }
  const registered = registeredTokenProgram(ctx, mint);
  if (registered != null) {
    return registered;
  }
  const info = await ctx.rpc.getAccountInfo(mint, { commitment: ctx.commitment, encoding: 'base64' }).send();
  if (info.value == null) {
    return null;
  }
  return accountOwner(info.value) ?? null;
}

export async function kaminoAtaExists(ctx: KaminoInstructionContext, args: KaminoAtaExistsArgs): Promise<boolean> {
  const owner = args.owner ?? requireClientPrimary(ctx);
  const tokenProgram = await resolveTokenProgram(ctx, args.mint, args.tokenProgram);
  if (tokenProgram == null) {
    return false;
  }
  const ata = await clientPrimaryAta({
    clientPrimaryAccount: owner,
    mint: args.mint,
    tokenProgram,
  });
  const info = await ctx.rpc.getAccountInfo(ata, { commitment: ctx.commitment, encoding: 'base64' }).send();
  return info.value != null && accountOwner(info.value) === tokenProgram;
}

async function accountExistence(
  ctx: KaminoInstructionContext,
  address: Address,
  expectedOwner: Address,
): Promise<{ address: Address; exists: boolean; expectedOwner: Address }> {
  const info = await ctx.rpc.getAccountInfo(address, { commitment: ctx.commitment, encoding: 'base64' }).send();
  return {
    address,
    expectedOwner,
    exists: info.value != null && accountOwner(info.value) === expectedOwner,
  };
}

async function farmAccountsExistence(
  ctx: KaminoInstructionContext,
  farm: KaminoFarmContext,
): Promise<KaminoInstrumentAccountsExistResponse['farms']['assetCollateral']> {
  if (!farm.hasFarm) {
    return null;
  }
  const [reserveFarmState, obligationFarm] = await Promise.all([
    accountExistence(ctx, farm.reserveFarmState, FARMS_PROGRAM_ID),
    accountExistence(ctx, farm.obligationFarm, FARMS_PROGRAM_ID),
  ]);
  return { reserveFarmState, obligationFarm };
}

export async function kaminoInstrumentAccountsExist(
  ctx: KaminoInstructionContext,
  args: KaminoInstrumentAccountsExistArgs,
  kaminoCtx: KaminoContext,
): Promise<KaminoInstrumentAccountsExistResponse> {
  const instr = requireInstrument(ctx, args.instrId);
  const { assetMint, crncyMint } = instrumentMints(ctx, instr);
  validateKaminoReserveForMint(kaminoCtx.collateralReserve, assetMint, kaminoCtx.lendingMarket, 'Collateral');
  validateKaminoReserveForMint(kaminoCtx.debtReserve, crncyMint, kaminoCtx.lendingMarket, 'Debt');
  const [assetAta, crncyAta] = await Promise.all([
    clientPrimaryAta({
      clientPrimaryAccount: kaminoCtx.clientPrimaryAccount,
      mint: assetMint,
      tokenProgram: kaminoCtx.collateralReserve.tokenProgram,
    }),
    clientPrimaryAta({
      clientPrimaryAccount: kaminoCtx.clientPrimaryAccount,
      mint: crncyMint,
      tokenProgram: kaminoCtx.debtReserve.tokenProgram,
    }),
  ]);
  const [assetAtaResult, crncyAtaResult, assetCollateral, assetLiquidity, crncyCollateral, crncyLiquidity] =
    await Promise.all([
      accountExistence(ctx, assetAta, kaminoCtx.collateralReserve.tokenProgram),
      accountExistence(ctx, crncyAta, kaminoCtx.debtReserve.tokenProgram),
      farmAccountsExistence(ctx, kaminoCtx.collateralReserve.collateralFarm),
      farmAccountsExistence(ctx, kaminoCtx.collateralReserve.liquidityFarm),
      farmAccountsExistence(ctx, kaminoCtx.debtReserve.collateralFarm),
      farmAccountsExistence(ctx, kaminoCtx.debtReserve.liquidityFarm),
    ]);
  const farmGroups = [assetCollateral, assetLiquidity, crncyCollateral, crncyLiquidity];
  const allFarmAccountsExist = farmGroups.every(
    (farm) => farm == null || (farm.reserveFarmState.exists && farm.obligationFarm.exists),
  );
  return {
    assetAta: assetAtaResult,
    crncyAta: crncyAtaResult,
    farms: {
      assetCollateral,
      assetLiquidity,
      crncyCollateral,
      crncyLiquidity,
    },
    allExist: assetAtaResult.exists && crncyAtaResult.exists && allFarmAccountsExist,
  };
}

function reserveInfo(context: KaminoContext, reserve: Address): KaminoReserveInfo | null {
  if (reserve === context.collateralReserve.address) return context.collateralReserve;
  if (reserve === context.debtReserve.address) return context.debtReserve;
  return context.extraReserves.find((entry) => entry.address === reserve) ?? null;
}

function reserveDecimals(context: KaminoContext, reserve: Address): number {
  return reserveInfo(context, reserve)?.mintDecimals ?? 0;
}

function deriverseTokenId(ctx: KaminoInstructionContext | null, mint: Address): number | null {
  if (ctx == null) return null;
  for (const [id, token] of ctx.tokens) {
    if (token.address === mint) return id;
  }
  return null;
}

function collateralToLiquidityRaw(reserve: KaminoReserveInfo | null, collateralAmountRaw: number): number {
  if (reserve == null || collateralAmountRaw === 0) return collateralAmountRaw;
  const collateralSupply = reserve.raw.collateralMintTotalSupply;
  const totalLiquiditySf = reserve.raw.totalLiquiditySfRaw;
  if (collateralSupply === 0 || totalLiquiditySf <= BigInt(0)) return collateralAmountRaw;
  const liquiditySf = (BigInt(collateralAmountRaw) * totalLiquiditySf) / BigInt(collateralSupply);
  return sfToIntegerFloor(liquiditySf);
}

function positionMetadata(
  ctx: KaminoInstructionContext | null,
  reserve: KaminoReserveInfo | null,
  reserveAddress: Address,
) {
  const liquidityMint = reserve?.liquidityMint ?? NULL_ADDRESS;
  return {
    reserve: reserveAddress,
    liquidityMint,
    collateralMint: reserve?.collateralMint ?? NULL_ADDRESS,
    tokenProgram: reserve?.tokenProgram ?? TOKEN_PROGRAM_ID,
    deriverseTokenId: deriverseTokenId(ctx, liquidityMint),
  };
}

function decodeKaminoClientState(
  obligation: Address,
  buffer: Buffer,
  context: KaminoContext,
  ctx: KaminoInstructionContext | null = null,
): KaminoClientStateResponse {
  const owner = readAddress(buffer, OBLIGATION_OWNER_OFFSET);
  const deposits = [];
  for (let i = 0; i < 8; i++) {
    const offset = OBLIGATION_DEPOSITS_OFFSET + i * OBLIGATION_COLLATERAL_SIZE;
    const reserve = readAddress(buffer, offset);
    const collateralAmountRaw = readU64(buffer, offset + OBLIGATION_COLLATERAL_DEPOSITED_AMOUNT_OFFSET);
    const marketValue = sfToNumber(readU128(buffer, offset + OBLIGATION_COLLATERAL_MARKET_VALUE_OFFSET));
    if (!isDefaultAddress(reserve) || collateralAmountRaw !== 0 || marketValue !== 0) {
      const info = reserveInfo(context, reserve);
      const decimals = reserveDecimals(context, reserve);
      const depositedAmountRaw = collateralToLiquidityRaw(info, collateralAmountRaw);
      deposits.push({
        ...positionMetadata(ctx, info, reserve),
        depositedAmount: depositedAmountRaw / 10 ** decimals,
        depositedAmountRaw,
        collateralAmount: collateralAmountRaw / 10 ** decimals,
        collateralAmountRaw,
        depositMarketValue: marketValue,
        borrowedAmount: 0,
        borrowedAmountRaw: 0,
        borrowMarketValue: 0,
      });
    }
  }

  const borrows = [];
  for (let i = 0; i < 5; i++) {
    const offset = OBLIGATION_BORROWS_OFFSET + i * OBLIGATION_LIQUIDITY_SIZE;
    const reserve = readAddress(buffer, offset);
    const borrowedAmountSf = readU128(buffer, offset + OBLIGATION_BORROWED_AMOUNT_SF_OFFSET);
    const amountRaw = sfToIntegerCeil(borrowedAmountSf);
    const marketValue = sfToNumber(readU128(buffer, offset + OBLIGATION_BORROW_MARKET_VALUE_SF_OFFSET));
    if (!isDefaultAddress(reserve) || amountRaw !== 0 || marketValue !== 0) {
      const info = reserveInfo(context, reserve);
      const decimals = reserveDecimals(context, reserve);
      borrows.push({
        ...positionMetadata(ctx, info, reserve),
        depositedAmount: 0,
        depositedAmountRaw: 0,
        collateralAmount: 0,
        collateralAmountRaw: 0,
        depositMarketValue: 0,
        borrowedAmount: amountRaw / 10 ** decimals,
        borrowedAmountRaw: amountRaw,
        borrowMarketValue: marketValue,
      });
    }
  }

  const totalDepositValue = sfToNumber(readU128(buffer, OBLIGATION_DEPOSITED_VALUE_SF_OFFSET));
  const borrowFactorAdjustedDebtValue = sfToNumber(
    readU128(buffer, OBLIGATION_BORROW_FACTOR_ADJUSTED_DEBT_VALUE_SF_OFFSET),
  );
  const aggregateTotalBorrowValue = sfToNumber(readU128(buffer, OBLIGATION_BORROWED_ASSETS_MARKET_VALUE_SF_OFFSET));
  const borrowLimit = sfToNumber(readU128(buffer, OBLIGATION_ALLOWED_BORROW_VALUE_SF_OFFSET));
  const unhealthyBorrowValue = sfToNumber(readU128(buffer, OBLIGATION_UNHEALTHY_BORROW_VALUE_SF_OFFSET));
  const fallbackBorrowValue = borrows.reduce((total, entry) => {
    if (entry.borrowMarketValue > 0) return total + entry.borrowMarketValue;
    const info = reserveInfo(context, entry.reserve);
    return total + entry.borrowedAmount * (info?.raw.marketPriceSf ?? 0);
  }, 0);
  const totalBorrowValue =
    aggregateTotalBorrowValue > 0 || fallbackBorrowValue === 0 ? aggregateTotalBorrowValue : fallbackBorrowValue;
  const effectiveBorrowFactorAdjustedDebtValue =
    borrowFactorAdjustedDebtValue > 0 || totalBorrowValue === 0 ? borrowFactorAdjustedDebtValue : totalBorrowValue;

  const collateralDeposit = deposits.find((entry) => entry.reserve === context.collateralReserve.address);
  let maxWithdrawEstimate: KaminoClientStateResponse['maxWithdrawEstimate'] = null;
  if (collateralDeposit != null) {
    let amountRaw = 0;
    if (collateralDeposit.depositedAmountRaw > 0) {
      if (borrows.length === 0) {
        amountRaw = collateralDeposit.depositedAmountRaw;
      } else if (collateralDeposit.depositMarketValue > 0) {
        const maxWithdrawValue =
          borrowLimit <= effectiveBorrowFactorAdjustedDebtValue
            ? 0
            : context.collateralReserve.loanToValuePct === 0
              ? collateralDeposit.depositMarketValue
              : (borrowLimit - effectiveBorrowFactorAdjustedDebtValue) /
                (context.collateralReserve.loanToValuePct / 100);
        const withdrawValue = Math.min(maxWithdrawValue, collateralDeposit.depositMarketValue);
        amountRaw = Math.min(
          collateralDeposit.depositedAmountRaw,
          Math.floor((withdrawValue / collateralDeposit.depositMarketValue) * collateralDeposit.depositedAmountRaw),
        );
      }
    }
    maxWithdrawEstimate = {
      reserve: context.collateralReserve.address,
      amountRaw,
      amount: amountRaw / 10 ** context.collateralReserve.mintDecimals,
      collateralAmountRaw:
        collateralDeposit.depositedAmountRaw === 0
          ? 0
          : Math.min(
              collateralDeposit.collateralAmountRaw,
              Math.floor((amountRaw / collateralDeposit.depositedAmountRaw) * collateralDeposit.collateralAmountRaw),
            ),
    };
  }

  return {
    obligation,
    exists: true,
    lendingMarket: context.lendingMarket,
    reserves: {
      collateralReserve: context.collateralReserve.address,
      debtReserve: context.debtReserve.address,
    },
    deposits,
    borrows,
    totalDepositValue,
    totalBorrowValue,
    borrowLimit,
    unhealthyBorrowValue,
    ltv: totalDepositValue === 0 ? null : totalBorrowValue / totalDepositValue,
    healthFactor: totalBorrowValue === 0 || unhealthyBorrowValue === 0 ? null : unhealthyBorrowValue / totalBorrowValue,
    liquidationBuffer:
      totalBorrowValue === 0 || unhealthyBorrowValue === 0 ? null : unhealthyBorrowValue - totalBorrowValue,
    maxWithdrawEstimate,
    raw: {
      owner,
      depositedValueSf: totalDepositValue,
      borrowedAssetsMarketValueSf: aggregateTotalBorrowValue,
      borrowFactorAdjustedDebtValueSf: borrowFactorAdjustedDebtValue,
      allowedBorrowValueSf: borrowLimit,
      unhealthyBorrowValueSf: unhealthyBorrowValue,
    },
  };
}

export async function getKaminoClientState(
  ctx: KaminoInstructionContext,
  args: GetKaminoClientStateArgs,
  context: KaminoContext,
): Promise<KaminoClientStateResponse> {
  const obligation = args.obligation ?? context.obligation;
  const info = await ctx.rpc.getAccountInfo(obligation, { commitment: ctx.commitment, encoding: 'base64' }).send();
  if (info.value == null || accountOwner(info.value) !== KLEND_PROGRAM_ID) {
    return {
      obligation,
      exists: false,
      lendingMarket: context.lendingMarket,
      reserves: {
        collateralReserve: context.collateralReserve.address,
        debtReserve: context.debtReserve.address,
      },
      deposits: [],
      borrows: [],
      totalDepositValue: 0,
      totalBorrowValue: 0,
      borrowLimit: 0,
      unhealthyBorrowValue: 0,
      ltv: null,
      healthFactor: null,
      liquidationBuffer: null,
      maxWithdrawEstimate: null,
      raw: {},
    };
  }
  const buffer = dataToBuffer(info.value.data);
  if (
    buffer.length < OBLIGATION_UNHEALTHY_BORROW_VALUE_SF_OFFSET + 16 ||
    !buffer.subarray(0, 8).equals(OBLIGATION_DISCRIMINATOR)
  ) {
    throw new Error(`Invalid Kamino obligation layout: ${obligation}`);
  }
  return decodeKaminoClientState(obligation, buffer, context, ctx);
}

export function getKaminoClientStateFromData(args: {
  obligation: Address;
  obligationData: Base64EncodedDataResponse;
  context: KaminoContext;
  ctx?: KaminoInstructionContext;
}): KaminoClientStateResponse {
  const buffer = dataToBuffer(args.obligationData);
  if (
    buffer.length < OBLIGATION_UNHEALTHY_BORROW_VALUE_SF_OFFSET + 16 ||
    !buffer.subarray(0, 8).equals(OBLIGATION_DISCRIMINATOR)
  ) {
    throw new Error(`Invalid Kamino obligation layout: ${args.obligation}`);
  }
  return decodeKaminoClientState(args.obligation, buffer, args.context, args.ctx ?? null);
}
