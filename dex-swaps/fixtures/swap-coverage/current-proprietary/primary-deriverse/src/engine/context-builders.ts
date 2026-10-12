import { AccountRole } from '@solana/kit';

import { AccountType } from '../types/enums';
import { InstrAccountHeaderModel } from '../structure_models';
import { getInstrAccountByTag, AccountHelperContext } from './account-helpers';
import { AccountMeta } from '../types';

/**
 * Build spot trading context accounts
 */
async function getSpotContext(
  ctx: AccountHelperContext,
  instrAccountHeaderModel: InstrAccountHeaderModel,
): Promise<AccountMeta[]> {
  const args = {
    assetTokenId: instrAccountHeaderModel.assetTokenId,
    crncyTokenId: instrAccountHeaderModel.crncyTokenId,
  };
  return [
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.INSTR }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_BIDS_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_ASKS_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_BID_ORDERS }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_ASK_ORDERS }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_LINES }),
      role: AccountRole.WRITABLE,
    },
    { address: instrAccountHeaderModel.mapsAddress, role: AccountRole.WRITABLE },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_CLIENT_INFOS }),
      role: AccountRole.WRITABLE,
    },
  ];
}

/**
 * Build perp trading context accounts
 */
async function getPerpContext(
  ctx: AccountHelperContext,
  instrAccountHeaderModel: InstrAccountHeaderModel,
): Promise<AccountMeta[]> {
  const args = {
    assetTokenId: instrAccountHeaderModel.assetTokenId,
    crncyTokenId: instrAccountHeaderModel.crncyTokenId,
  };
  return [
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.INSTR }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_BIDS_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_ASKS_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_BID_ORDERS }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_ASK_ORDERS }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_LINES }),
      role: AccountRole.WRITABLE,
    },
    { address: instrAccountHeaderModel.perpMapsAddress, role: AccountRole.WRITABLE },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_CLIENT_INFOS }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_CLIENT_INFOS2 }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_CLIENT_INFOS3 }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_CLIENT_INFOS4 }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_CLIENT_INFOS5 }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_LONG_PX_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.PERP_SHORT_PX_TREE }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, {
        ...args,
        tag: AccountType.PERP_REBALANCE_TIME_TREE,
      }),
      role: AccountRole.WRITABLE,
    },
  ];
}

/**
 * Build one-sided spot trading context accounts
 * side: 0 = bid, 1 = ask
 */
async function getSpotOneSidedContext(
  ctx: AccountHelperContext,
  instrAccountHeaderModel: InstrAccountHeaderModel,
  side: number,
): Promise<AccountMeta[]> {
  const args = {
    assetTokenId: instrAccountHeaderModel.assetTokenId,
    crncyTokenId: instrAccountHeaderModel.crncyTokenId,
  };
  const treeTag = side === 0 ? AccountType.SPOT_BIDS_TREE : AccountType.SPOT_ASKS_TREE;
  const ordersTag = side === 0 ? AccountType.SPOT_BID_ORDERS : AccountType.SPOT_ASK_ORDERS;
  return [
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.INSTR }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: treeTag }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: ordersTag }),
      role: AccountRole.WRITABLE,
    },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_LINES }),
      role: AccountRole.WRITABLE,
    },
    { address: instrAccountHeaderModel.mapsAddress, role: AccountRole.WRITABLE },
    {
      address: await getInstrAccountByTag(ctx, { ...args, tag: AccountType.SPOT_CLIENT_INFOS }),
      role: AccountRole.WRITABLE,
    },
  ];
}

export { getSpotContext, getSpotOneSidedContext, getPerpContext };
