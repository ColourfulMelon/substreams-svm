export function newOperatorData(tag: number, version: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(version, 4);
  return buf;
}

export function newRootAccountData(tag: number, privateMode: number, version: number, lutSlot: number): Buffer {
  let buf = Buffer.alloc(12);
  buf.writeUint8(tag, 0);
  buf.writeUint8(privateMode, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(version, 4);
  buf.writeUint32LE(lutSlot, 8);
  return buf;
}

export function newSpotOrderData(tag: number, ioc: number, orderType: number, side: number, instrId: number, price: number, amount: number, edgePrice: number): Buffer {
  let buf = Buffer.alloc(32);
  buf.writeUint8(tag, 0);
  buf.writeUint8(ioc, 1);
  buf.writeUint8(orderType, 2);
  buf.writeUint8(side, 3);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(price)), 8);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 16);
  buf.writeBigInt64LE(BigInt(Math.floor(edgePrice)), 24);
  return buf;
}

export function newPerpOrderData(tag: number, ioc: number, leverage: number, orderType: number, side: number, instrId: number, price: number, amount: number, edgePrice: number): Buffer {
  let buf = Buffer.alloc(40);
  buf.writeUint8(tag, 0);
  buf.writeUint8(ioc, 1);
  buf.writeUint8(leverage, 2);
  buf.writeUint8(orderType, 3);
  buf.writeUint8(side, 4);
  buf.writeUint8(0, 5);
  buf.writeUint16LE(0, 6);
  buf.writeUint32LE(0, 8);
  buf.writeUint32LE(instrId, 12);
  buf.writeBigInt64LE(BigInt(Math.floor(price)), 16);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 24);
  buf.writeBigInt64LE(BigInt(Math.floor(edgePrice)), 32);
  return buf;
}

export function perpChangeLeverageData(tag: number, leverage: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(leverage, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function perpStatisticsResetData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function spotOrderCancelData(tag: number, side: number, instrId: number, orderId: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(side, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(orderId)), 8);
  return buf;
}

export function spotMassCancelData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function spotLpData(tag: number, side: number, instrId: number, amount: number, minPrice: number, maxPrice: number): Buffer {
  let buf = Buffer.alloc(32);
  buf.writeUint8(tag, 0);
  buf.writeUint8(side, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  buf.writeBigInt64LE(BigInt(Math.floor(minPrice)), 16);
  buf.writeBigInt64LE(BigInt(Math.floor(maxPrice)), 24);
  return buf;
}

export function newInstrumentData(tag: number, mask: number, crncyTokenId: number, lutSlot: number, price: number, minQty: number, fixedFeeRate: number): Buffer {
  let buf = Buffer.alloc(40);
  buf.writeUint8(tag, 0);
  buf.writeUint8(mask, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(0, 4);
  buf.writeUint32LE(crncyTokenId, 8);
  buf.writeUint32LE(lutSlot, 12);
  buf.writeBigInt64LE(BigInt(Math.floor(price)), 16);
  buf.writeBigInt64LE(BigInt(Math.floor(minQty)), 24);
  buf.writeDoubleLE(fixedFeeRate, 32);
  return buf;
}

export function depositData(tag: number, competitionId: number, depositAll: number, tokenId: number, amount: number, lutSlot: number, refId: number, customId: number): Buffer {
  let buf = Buffer.alloc(32);
  buf.writeUint8(tag, 0);
  buf.writeUint8(competitionId, 1);
  buf.writeUint8(depositAll, 2);
  buf.writeUint8(0, 3);
  buf.writeUint32LE(tokenId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  buf.writeUint32LE(lutSlot, 16);
  buf.writeUint32LE(refId, 20);
  buf.writeBigInt64LE(BigInt(Math.floor(customId)), 24);
  return buf;
}

export function feesDepositData(tag: number, tokenId: number, amount: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(tokenId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  return buf;
}

export function feesWithdrawData(tag: number, tokenId: number, amount: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(tokenId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  return buf;
}

export function perpDepositData(tag: number, instrId: number, amount: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  return buf;
}

export function moveSpotAvailFundsData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function perpWithdrawData(tag: number, instrId: number, amount: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  return buf;
}

export function withdrawData(tag: number, tokenId: number, amount: number, customId: number): Buffer {
  let buf = Buffer.alloc(24);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(tokenId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  buf.writeBigInt64LE(BigInt(Math.floor(customId)), 16);
  return buf;
}

export function swapData(tag: number, inputCrncy: number, instrId: number, price: number, amount: number, minAmountOut: number): Buffer {
  let buf = Buffer.alloc(32);
  buf.writeUint8(tag, 0);
  buf.writeUint8(inputCrncy, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(price)), 8);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 16);
  buf.writeBigInt64LE(BigInt(Math.floor(minAmountOut)), 24);
  return buf;
}

export function spotQuotesReplaceData(tag: number, bump: number, orderType: number, bailOnOrderNotFound: number, mask: number, instrId: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(bump, 1);
  buf.writeUint8(orderType, 2);
  buf.writeUint8(bailOnOrderNotFound, 3);
  buf.writeUint16LE(mask, 4);
  buf.writeUint16LE(0, 6);
  buf.writeUint32LE(instrId, 8);
  buf.writeUint32LE(0, 12);
  return buf;
}

export function spotQuotesReplaceDataV2(
  tag: number,
  bump: number,
  orderType: number,
  config: number,
  quotesSides: number,
  instrId: number,
  quotesAmount: number,
  priceTick: number,
  quantityTick: number,
): Buffer {
  let buf = Buffer.alloc(32);
  buf.writeUint8(tag, 0);
  buf.writeUint8(bump, 1);
  buf.writeUint8(orderType, 2);
  buf.writeUint8(config, 3);
  buf.writeUint32LE(quotesSides, 4);
  buf.writeUint32LE(instrId, 8);
  buf.writeUint8(quotesAmount, 12);
  buf.writeUint8(0, 13);
  buf.writeUint16LE(0, 14);
  buf.writeBigInt64LE(BigInt(priceTick), 16);
  buf.writeBigInt64LE(BigInt(quantityTick), 24);
  return buf;
}

export function perpQuotesReplaceData(tag: number, bump: number, orderType: number, bailOnOrderNotFound: number, mask: number, instrId: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(bump, 1);
  buf.writeUint8(orderType, 2);
  buf.writeUint8(bailOnOrderNotFound, 3);
  buf.writeUint16LE(mask, 4);
  buf.writeUint16LE(0, 6);
  buf.writeUint32LE(instrId, 8);
  buf.writeUint32LE(0, 12);
  return buf;
}

export function votingData(tag: number, choice: number, votingCounter: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(choice, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(votingCounter, 4);
  return buf;
}

export function airdropData(tag: number, ratio: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(0, 4);
  buf.writeDoubleLE(ratio, 8);
  return buf;
}

export function upgradeToPerpData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function setInstrReadyForPerpUpgradeData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function perpOrderCancelData(tag: number, side: number, instrId: number, orderId: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(side, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(orderId)), 8);
  return buf;
}

export function perpMassCancelData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function changeRefProgramData(tag: number, refProgramDuration: number, refLinkDuration: number, refDiscount: number, refRatio: number): Buffer {
  let buf = Buffer.alloc(32);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(0, 4);
  buf.writeUint32LE(refProgramDuration, 8);
  buf.writeUint32LE(refLinkDuration, 12);
  buf.writeDoubleLE(refDiscount, 16);
  buf.writeDoubleLE(refRatio, 24);
  return buf;
}

export function buyMarketSeatData(tag: number, instrId: number, edgePrice: number, amount: number): Buffer {
  let buf = Buffer.alloc(24);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(edgePrice)), 8);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 16);
  return buf;
}

export function sellMarketSeatData(tag: number, instrId: number, edgePrice: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(edgePrice)), 8);
  return buf;
}

export function newPrivateClient(tag: number, expirationTime: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(expirationTime, 4);
  return buf;
}

export function pointsProgramExpiration(tag: number, newExpirationTime: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(newExpirationTime, 4);
  return buf;
}

export function setVarianceData(tag: number, instrId: number, variance: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeDoubleLE(variance, 8);
  return buf;
}

export function changeDenominatorData(tag: number, baseCrncyId: number, denominator: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(baseCrncyId, 4);
  buf.writeDoubleLE(denominator, 8);
  return buf;
}

export function newBaseCrncyData(tag: number, denominator: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(0, 4);
  buf.writeDoubleLE(denominator, 8);
  return buf;
}

export function perpClientsProcessingData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function setSeatPurchasingFeeData(tag: number, fee: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(0, 4);
  buf.writeDoubleLE(fee, 8);
  return buf;
}

export function changeVotingData(tag: number, newChoice: number, votingCounter: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(newChoice, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(votingCounter, 4);
  return buf;
}

export function garbageCollectorData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function activateClientRefProgramData(tag: number, refId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(refId, 4);
  return buf;
}

export function cleanCandlesData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function extendCandlesData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function vmInitWithdrawData(tag: number, tokenId: number, amount: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(tokenId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  return buf;
}

export function vmChangeWhitelistData(tag: number, mask: number, whitelist: number[]): Buffer {
  let buf = Buffer.alloc(40);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(mask, 4);
  for (let i = 0; i < 8; i++) { buf.writeUint32LE(whitelist[i] ?? 0, 8 + i * 4); }
  return buf;
}

export function withdrawSwapFeesData(tag: number, instrId: number, amount: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  return buf;
}

export function setSAMMinQtyData(tag: number, instrId: number, minQty: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(minQty)), 8);
  return buf;
}

export function changeSAMFeesPolicyData(tag: number, samFeeType: number, instrId: number, feeRate: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(samFeeType, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeDoubleLE(feeRate, 8);
  return buf;
}

export function suspendInstrumentData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function vmDirectWithdrawData(tag: number, tokenId: number, amount: number): Buffer {
  let buf = Buffer.alloc(16);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(tokenId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(amount)), 8);
  return buf;
}

export function kaminoInitObligationData(tag: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(0, 4);
  return buf;
}

export function kaminoInitInstrumentData(tag: number, instrId: number): Buffer {
  let buf = Buffer.alloc(8);
  buf.writeUint8(tag, 0);
  buf.writeUint8(0, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  return buf;
}

export function kaminoChangePositionData(
  tag: number,
  flags: number,
  instrId: number,
  borrowDelta: number,
  collateralDelta: number,
  customId: number,
): Buffer {
  let buf = Buffer.alloc(32);
  buf.writeUint8(tag, 0);
  buf.writeUint8(flags, 1);
  buf.writeUint16LE(0, 2);
  buf.writeUint32LE(instrId, 4);
  buf.writeBigInt64LE(BigInt(Math.floor(borrowDelta)), 8);
  buf.writeBigInt64LE(BigInt(Math.floor(collateralDelta)), 16);
  buf.writeBigInt64LE(BigInt(Math.floor(customId)), 24);
  return buf;
}

export function vmInitActivateData(tag: number, multisig: number): Buffer {
  let buf = Buffer.alloc(2);
  buf.writeUint8(tag, 0);
  buf.writeUint8(multisig, 1);
  return buf;
}

export function setForeignDepositData(tag: number, foreignDeposit: number): Buffer {
  let buf = Buffer.alloc(2);
  buf.writeUint8(tag, 0);
  buf.writeUint8(foreignDeposit, 1);
  return buf;
}
