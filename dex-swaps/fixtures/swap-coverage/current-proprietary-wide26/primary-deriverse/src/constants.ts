import { address } from '@solana/kit';
import { CandlesHeaderModel } from './structure_models';

export const VERSION = 1;
export const PROGRAM_ID = address('DRVSpZ2YUYYKgZP8XtLhAGtT1zYSCKzeHfb4DgRnrgqD');
export const MARKET_DEPTH = 20;

export const ADDRESS_LOOKUP_TABLE_PROGRAM_ID = address('AddressLookupTab1e1111111111111111111111111');
export const SYSTEM_PROGRAM_ID = address('11111111111111111111111111111111');
export const TOKEN_PROGRAM_ID = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022_PROGRAM_ID = address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const ASSOCIATED_TOKEN_PROGRAM_ID = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
export const MAIN_KAMINO_MARKET = address('7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF');
export const MAIN_KAMINO_MARKET_LUT = address('284iwGtA9X9aLy3KsyV8uT2pXLARhYbiSi5SiM2g47M2');
export const KLEND_PROGRAM_ID = address('KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD');
export const FARMS_PROGRAM_ID = address('FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr');
export const STANDARD_MAPS_SIZE = 42184 + CandlesHeaderModel.LENGTH; // spot::memory_maps::MAPS_SIZE + CandlesHeader
export const EXTENDED_MAPS_SIZE = 68712 + CandlesHeaderModel.LENGTH; // extended_spot::memory_maps::MAPS_SIZE + CandlesHeader
export const DF = 1000000000;

const LP_DEC_UI = 10000;
const FEE_RATE_STEP_UI = 0.0005;
const POOL_RATIO_STEP_UI = 0.025;

export let dec = DF;
export let lpDec = LP_DEC_UI;
export let feeRateStep = FEE_RATE_STEP_UI;
export let poolRatioStep = POOL_RATIO_STEP_UI;

export const nullOrder = 0xffff;

export function setDecimals(uiNumbers: boolean): void {
  dec = uiNumbers ? DF : 1;
  lpDec = uiNumbers ? LP_DEC_UI : 1;
  feeRateStep = uiNumbers ? FEE_RATE_STEP_UI : 1;
  poolRatioStep = uiNumbers ? POOL_RATIO_STEP_UI : 1;
}
