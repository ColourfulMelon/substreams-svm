"use strict";Object.defineProperty(exports, "__esModule", {value: true}); function _nullishCoalesce(lhs, rhsFn) { if (lhs != null) { return lhs; } else { return rhsFn(); } } function _optionalChain(ops) { let lastAccessLHS = undefined; let value = ops[0]; let i = 1; while (i < ops.length) { const op = ops[i]; const fn = ops[i + 1]; i += 2; if ((op === 'optionalAccess' || op === 'optionalCall') && value == null) { return undefined; } if (op === 'access' || op === 'optionalAccess') { lastAccessLHS = value; value = fn(value); } else if (op === 'call' || op === 'optionalCall') { value = fn((...args) => value.call(lastAccessLHS, ...args)); lastAccessLHS = undefined; } } return value; }// src/hadron.ts





var _spltoken = require('@solana/spl-token');

// src/helpers/sol.ts


var _web3js = require('@solana/web3.js');










function isNativeMint(mint) {
  return mint.equals(_spltoken.NATIVE_MINT);
}
function wrapSolInstructions(user, lamports) {
  const ata = _spltoken.getAssociatedTokenAddressSync.call(void 0, _spltoken.NATIVE_MINT, user, true, _spltoken.TOKEN_PROGRAM_ID);
  return [
    _spltoken.createAssociatedTokenAccountIdempotentInstruction.call(void 0, 
      user,
      ata,
      user,
      _spltoken.NATIVE_MINT,
      _spltoken.TOKEN_PROGRAM_ID
    ),
    _web3js.SystemProgram.transfer({
      fromPubkey: user,
      toPubkey: ata,
      lamports
    }),
    _spltoken.createSyncNativeInstruction.call(void 0, ata, _spltoken.TOKEN_PROGRAM_ID)
  ];
}
function createWsolAtaInstruction(user) {
  const ata = _spltoken.getAssociatedTokenAddressSync.call(void 0, _spltoken.NATIVE_MINT, user, true, _spltoken.TOKEN_PROGRAM_ID);
  return _spltoken.createAssociatedTokenAccountIdempotentInstruction.call(void 0, 
    user,
    ata,
    user,
    _spltoken.NATIVE_MINT,
    _spltoken.TOKEN_PROGRAM_ID
  );
}
function unwrapSolInstructions(user) {
  const ata = _spltoken.getAssociatedTokenAddressSync.call(void 0, _spltoken.NATIVE_MINT, user, true, _spltoken.TOKEN_PROGRAM_ID);
  return [
    _spltoken.createCloseAccountInstruction.call(void 0, ata, user, user, [], _spltoken.TOKEN_PROGRAM_ID),
    _spltoken.createAssociatedTokenAccountIdempotentInstruction.call(void 0, 
      user,
      ata,
      user,
      _spltoken.NATIVE_MINT,
      _spltoken.TOKEN_PROGRAM_ID
    )
  ];
}
function buildFeeRecipientAtaIxs(payer, feeRecipient, mintX, mintY, tokenProgramX = _spltoken.TOKEN_PROGRAM_ID, tokenProgramY = _spltoken.TOKEN_PROGRAM_ID) {
  const ataX = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintX, feeRecipient, true, tokenProgramX);
  const ataY = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintY, feeRecipient, true, tokenProgramY);
  return [
    _spltoken.createAssociatedTokenAccountIdempotentInstruction.call(void 0, 
      payer,
      ataX,
      feeRecipient,
      mintX,
      tokenProgramX
    ),
    _spltoken.createAssociatedTokenAccountIdempotentInstruction.call(void 0, 
      payer,
      ataY,
      feeRecipient,
      mintY,
      tokenProgramY
    )
  ];
}

// src/constants/index.ts

var HADRON_PROGRAM_ID = new (0, _web3js.PublicKey)(
  "HADRoNbLovyqhCsocfYQYB7QdfCAAinN9HTePvBCVDQ8"
);
var CONFIG_SEED = Buffer.from("hadron-config");
var MIDPRICE_ORACLE_SEED = Buffer.from("hadron-midprice");
var CURVE_META_SEED = Buffer.from("hadron-curve-meta");
var CURVE_PREFABS_SEED = Buffer.from("hadron-curve-prefabs");
var CURVE_UPDATES_SEED = Buffer.from("hadron-curve-updates");
var FEE_CONFIG_SEED = Buffer.from("fee_config");
var SPREAD_CONFIG_SEED = Buffer.from("spread_config");
var CONFIG_SIZE = 724;
var MIDPRICE_ORACLE_SIZE = 128;
var CURVE_META_SIZE = 56;
var FEE_CONFIG_SIZE = 72;
var ASSET_FEE_PAIR_COUNT_SIZE = 2;
var ASSET_FEE_ENTRY_SIZE = 68;
var MAX_ASSET_FEES = 64;
var CURVE_UPDATES_SIZE = 998;
var CURVE_SIDE_HEADER = 8;
var CURVE_POINT_LEN = 24;
var CURVE_UPDATE_OP_DATA_SIZE = 33;
var SPREAD_TRIGGER_IX_DATA_SIZE = 66;
var DEFAULT_MAX_PREFAB_SLOTS = 2;
var DEFAULT_MAX_CURVE_POINTS = 16;
var ABSOLUTE_MAX_PREFAB_SLOTS = 16;
var ABSOLUTE_MAX_CURVE_POINTS = 128;
var MAX_SETCURVE_POINTS = 32;
var MAX_CURVE_UPDATE_OPS = 28;
var MAX_SPREAD_ACCOUNTS = 100;
var Q32_ONE = BigInt(1) << BigInt(32);
var Discriminator = {
  Initialize: 0,
  Deposit: 1,
  Withdraw: 2,
  SwapExactIn: 3,
  SetCurve: 4,
  UpdateMidprice: 5,
  InitializeFeeConfig: 6,
  UpdateFeeConfig: 7,
  SetRiskCurve: 8,
  UpdateBaseSpread: 9,
  UpdateMidpriceAndBaseSpread: 10,
  SwitchPriceCurve: 11,
  SwitchRiskCurve: 12,
  InitializeSpreadConfig: 13,
  UpdateSpreadConfig: 14,
  UpdateDeltaStaleness: 15,
  NominateAuthority: 16,
  AcceptAuthority: 17,
  SubmitCurveUpdates: 18,
  ApplyCurveUpdates: 19,
  ClosePool: 20,
  SetPoolState: 21,
  AllocateCurvePrefabs: 22,
  // disc 23 ("SetQuotingAuthority") removed in v2 — see RotateQuotingAuthority(35) below.
  RotateFeeAdmin: 24,
  InitializePoolFeeConfig: 25,
  UpdatePoolFeeConfig: 26,
  SetAssetFee: 27,
  ValidateCurveUpdates: 28,
  RemoveSpreadTriggers: 29,
  UpdateDrawdownGuard: 30,
  UpdateFactorClamp: 31,
  ClosePoolFeeConfig: 32,
  RotateDepositAuthority: 33,
  RotateWithdrawAuthority: 34,
  RotateQuotingAuthority: 35,
  RotateSpreadAdmin: 36,
  RotatePauseAuthority: 37,
  PokeDrawdownGuard: 38
};
var POINT_DATA_SIZE = 21;
var CURVE_UPDATE_OP_SIZE = 33;
function curvePrefabsSize(maxSlots, maxPoints) {
  return 32 + 4 * maxSlots * (CURVE_SIDE_HEADER + maxPoints * CURVE_POINT_LEN);
}

// src/helpers/derive.ts


function seedToBuffer(seed) {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(seed);
  return buf;
}
function getConfigAddress(seed, mintX, mintY, programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync(
    [CONFIG_SEED, seedToBuffer(seed), mintX.toBuffer(), mintY.toBuffer()],
    programId
  );
}
function getMidpriceOracleAddress(seed, mintX, mintY, programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync(
    [
      MIDPRICE_ORACLE_SEED,
      seedToBuffer(seed),
      mintX.toBuffer(),
      mintY.toBuffer()
    ],
    programId
  );
}
function getCurveMetaAddress(seed, mintX, mintY, programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync(
    [CURVE_META_SEED, seedToBuffer(seed), mintX.toBuffer(), mintY.toBuffer()],
    programId
  );
}
function getCurvePrefabsAddress(creator, seed, mintX, mintY, programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync(
    [
      CURVE_PREFABS_SEED,
      creator.toBuffer(),
      seedToBuffer(seed),
      mintX.toBuffer(),
      mintY.toBuffer()
    ],
    programId
  );
}
function getCurveUpdatesAddress(seed, mintX, mintY, programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync(
    [
      CURVE_UPDATES_SEED,
      seedToBuffer(seed),
      mintX.toBuffer(),
      mintY.toBuffer()
    ],
    programId
  );
}
function getFeeConfigAddress(programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync([FEE_CONFIG_SEED], programId);
}
function getPoolFeeConfigAddress(configPda, programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync(
    [FEE_CONFIG_SEED, configPda.toBuffer()],
    programId
  );
}
function getSpreadConfigAddress(configPda, programId = HADRON_PROGRAM_ID) {
  return _web3js.PublicKey.findProgramAddressSync(
    [SPREAD_CONFIG_SEED, configPda.toBuffer()],
    programId
  );
}
function derivePoolAddresses(creator, seed, mintX, mintY, tokenProgramX, tokenProgramY, programId = HADRON_PROGRAM_ID) {
  const [config, configBump] = getConfigAddress(seed, mintX, mintY, programId);
  const [midpriceOracle] = getMidpriceOracleAddress(
    seed,
    mintX,
    mintY,
    programId
  );
  const [curveMeta] = getCurveMetaAddress(seed, mintX, mintY, programId);
  const [curvePrefabs] = getCurvePrefabsAddress(creator, seed, mintX, mintY, programId);
  const [curveUpdates] = getCurveUpdatesAddress(seed, mintX, mintY, programId);
  const vaultX = _spltoken.getAssociatedTokenAddressSync.call(void 0, 
    mintX,
    config,
    true,
    tokenProgramX
  );
  const vaultY = _spltoken.getAssociatedTokenAddressSync.call(void 0, 
    mintY,
    config,
    true,
    tokenProgramY
  );
  return {
    config,
    configBump,
    midpriceOracle,
    curveMeta,
    curvePrefabs,
    curveUpdates,
    vaultX,
    vaultY
  };
}

// src/helpers/math.ts
function toQ32(value) {
  return BigInt(Math.floor(value * Number(Q32_ONE)));
}
function fromQ32(q32) {
  const intPart = q32 >> 32n;
  const fracPart = q32 & 0xFFFFFFFFn;
  return Number(intPart) + Number(fracPart) / Number(Q32_ONE);
}
function pctToQ32(pct) {
  return toQ32(pct);
}
function spreadBpsToQ32(bps) {
  return toQ32(1 - bps / 1e4);
}
function spreadQ32ToBps(q32) {
  return (1 - fromQ32(q32)) * 1e4;
}
function pctBaseQ32(vaultX, vaultY, midpriceQ32) {
  if (midpriceQ32 === 0n) return Q32_ONE / 2n;
  const quoteInBase = (vaultY << 32n) / midpriceQ32;
  const total = vaultX + quoteInBase;
  if (total === 0n) return Q32_ONE / 2n;
  const pct = (vaultX << 32n) / total;
  return pct > Q32_ONE ? Q32_ONE : pct;
}

// src/accounts/index.ts


// src/types/index.ts
var PoolState = /* @__PURE__ */ ((PoolState2) => {
  PoolState2[PoolState2["Uninitialized"] = 0] = "Uninitialized";
  PoolState2[PoolState2["Initialized"] = 1] = "Initialized";
  PoolState2[PoolState2["Paused"] = 2] = "Paused";
  PoolState2[PoolState2["RejectSwaps"] = 3] = "RejectSwaps";
  PoolState2[PoolState2["Closed"] = 4] = "Closed";
  return PoolState2;
})(PoolState || {});
var OracleMode = /* @__PURE__ */ ((OracleMode2) => {
  OracleMode2[OracleMode2["Authority"] = 0] = "Authority";
  OracleMode2[OracleMode2["Relative"] = 1] = "Relative";
  return OracleMode2;
})(OracleMode || {});
var Interpolation = /* @__PURE__ */ ((Interpolation3) => {
  Interpolation3[Interpolation3["Step"] = 0] = "Step";
  Interpolation3[Interpolation3["Linear"] = 1] = "Linear";
  Interpolation3[Interpolation3["MarginalStep"] = 2] = "MarginalStep";
  Interpolation3[Interpolation3["Hyperbolic"] = 3] = "Hyperbolic";
  Interpolation3[Interpolation3["Quadratic"] = 4] = "Quadratic";
  Interpolation3[Interpolation3["Cubic"] = 5] = "Cubic";
  return Interpolation3;
})(Interpolation || {});
var CurveType = /* @__PURE__ */ ((CurveType2) => {
  CurveType2[CurveType2["PriceBid"] = 0] = "PriceBid";
  CurveType2[CurveType2["PriceAsk"] = 1] = "PriceAsk";
  CurveType2[CurveType2["RiskBid"] = 2] = "RiskBid";
  CurveType2[CurveType2["RiskAsk"] = 3] = "RiskAsk";
  return CurveType2;
})(CurveType || {});
var CurveXMode = /* @__PURE__ */ ((CurveXMode2) => {
  CurveXMode2[CurveXMode2["Native"] = 0] = "Native";
  CurveXMode2[CurveXMode2["Alternate"] = 1] = "Alternate";
  return CurveXMode2;
})(CurveXMode || {});
var RiskMode = /* @__PURE__ */ ((RiskMode2) => {
  RiskMode2[RiskMode2["Virtual"] = 0] = "Virtual";
  RiskMode2[RiskMode2["Integrated"] = 1] = "Integrated";
  return RiskMode2;
})(RiskMode || {});
var CurveUpdateOpKind = /* @__PURE__ */ ((CurveUpdateOpKind2) => {
  CurveUpdateOpKind2[CurveUpdateOpKind2["Edit"] = 0] = "Edit";
  CurveUpdateOpKind2[CurveUpdateOpKind2["Add"] = 1] = "Add";
  CurveUpdateOpKind2[CurveUpdateOpKind2["Remove"] = 2] = "Remove";
  return CurveUpdateOpKind2;
})(CurveUpdateOpKind || {});
var Side = /* @__PURE__ */ ((Side2) => {
  Side2[Side2["Bid"] = 0] = "Bid";
  Side2[Side2["Ask"] = 1] = "Ask";
  return Side2;
})(Side || {});

// src/accounts/index.ts
var DecoderError = class extends Error {
  constructor(kind, message) {
    super(`DecoderError[${kind}]: ${message}`);
    this.kind = kind;
    this.name = "DecoderError";
  }
};
function requireLen(data, min, what) {
  if (data.length < min) {
    throw new DecoderError(
      "DataTooShort",
      `${what} requires at least ${min} bytes, got ${data.length}`
    );
  }
}
function checkEnum(v, validValues, enumName) {
  if (validValues.indexOf(v) === -1) {
    throw new DecoderError(
      "InvalidEnum",
      `${enumName} byte ${v} is not in {${validValues.join(",")}}`
    );
  }
  return v;
}
var POOL_STATE_VALUES = [
  0 /* Uninitialized */,
  1 /* Initialized */,
  2 /* Paused */,
  3 /* RejectSwaps */,
  4 /* Closed */
];
var ORACLE_MODE_VALUES = [0 /* Authority */, 1 /* Relative */];
var INTERPOLATION_VALUES = [
  0 /* Step */,
  1 /* Linear */,
  2 /* MarginalStep */,
  3 /* Hyperbolic */,
  4 /* Quadratic */,
  5 /* Cubic */
];
var CURVE_X_MODE_VALUES = [0 /* Native */, 1 /* Alternate */];
var RISK_MODE_VALUES = [0 /* Virtual */, 1 /* Integrated */];
var CURVE_TYPE_VALUES = [
  0 /* PriceBid */,
  1 /* PriceAsk */,
  2 /* RiskBid */,
  3 /* RiskAsk */
];
var CURVE_UPDATE_OP_KIND_VALUES = [
  0 /* Edit */,
  1 /* Add */,
  2 /* Remove */
];
function decodeConfig(data) {
  requireLen(data, CONFIG_SIZE, "Config");
  const buf = Buffer.from(data);
  const state = checkEnum(buf.readUInt8(0), POOL_STATE_VALUES, "PoolState");
  const seed = buf.readBigUInt64LE(1);
  const authority = new (0, _web3js.PublicKey)(buf.subarray(9, 41));
  const mintX = new (0, _web3js.PublicKey)(buf.subarray(41, 73));
  const mintY = new (0, _web3js.PublicKey)(buf.subarray(73, 105));
  const configBump = buf.readUInt8(105);
  const curveMeta = new (0, _web3js.PublicKey)(buf.subarray(106, 138));
  const spreadConfigInitialized = buf.readUInt8(138) !== 0;
  const deltaStaleness = buf.readUInt8(139);
  const oracleMode = checkEnum(buf.readUInt8(140), ORACLE_MODE_VALUES, "OracleMode");
  const hasPoolFee = buf.readUInt8(141) !== 0;
  const lpSelfRotate = buf.readUInt8(142) !== 0;
  const pendingAuthority = new (0, _web3js.PublicKey)(buf.subarray(144, 176));
  const nominationExpiry = buf.readBigUInt64LE(176);
  const tokenProgramX = new (0, _web3js.PublicKey)(buf.subarray(184, 216));
  const tokenProgramY = new (0, _web3js.PublicKey)(buf.subarray(216, 248));
  const depositAuthority = new (0, _web3js.PublicKey)(buf.subarray(248, 280));
  const withdrawAuthority = new (0, _web3js.PublicKey)(buf.subarray(280, 312));
  const pauseAuthority = new (0, _web3js.PublicKey)(buf.subarray(312, 344));
  const hwmBaseCurrent = new Uint8Array(buf.subarray(344, 360));
  const hwmBasePrevious = new Uint8Array(buf.subarray(360, 376));
  const hwmQuoteCurrent = new Uint8Array(buf.subarray(376, 392));
  const hwmQuotePrevious = new Uint8Array(buf.subarray(392, 408));
  const lastRotationSlot = buf.readBigUInt64LE(408);
  const rotationIntervalSlots = buf.readUInt32LE(416);
  const thresholdBps = buf.readUInt16LE(420);
  const combinedFactorMaxQ32 = buf.readBigUInt64LE(422);
  const curvePrefabs = new (0, _web3js.PublicKey)(buf.subarray(430, 462));
  return {
    state,
    seed,
    authority,
    mintX,
    mintY,
    configBump,
    curveMeta,
    spreadConfigInitialized,
    deltaStaleness,
    oracleMode,
    hasPoolFee,
    lpSelfRotate,
    pendingAuthority,
    nominationExpiry,
    tokenProgramX,
    tokenProgramY,
    depositAuthority,
    withdrawAuthority,
    pauseAuthority,
    hwmBaseCurrent,
    hwmBasePrevious,
    hwmQuoteCurrent,
    hwmQuotePrevious,
    lastRotationSlot,
    rotationIntervalSlots,
    thresholdBps,
    combinedFactorMaxQ32,
    curvePrefabs
  };
}
function decodeMidpriceOracle(data) {
  requireLen(data, MIDPRICE_ORACLE_SIZE, "MidpriceOracle");
  const buf = Buffer.from(data);
  const authority = new (0, _web3js.PublicKey)(buf.subarray(0, 32));
  const sequence = buf.readBigUInt64LE(32);
  const midpriceQ32 = buf.readBigUInt64LE(40);
  const spreadFactorQ32 = buf.readBigUInt64LE(48);
  const lastUpdateSlot = buf.readBigUInt64LE(56);
  return { authority, sequence, midpriceQ32, spreadFactorQ32, lastUpdateSlot };
}
function decodeCurveMeta(data) {
  requireLen(data, CURVE_META_SIZE, "CurveMeta");
  const buf = Buffer.from(data);
  return {
    authority: new (0, _web3js.PublicKey)(buf.subarray(0, 32)),
    activePriceBidSlot: buf.readUInt8(32),
    activePriceAskSlot: buf.readUInt8(33),
    activeRiskBidSlot: buf.readUInt8(34),
    activeRiskAskSlot: buf.readUInt8(35),
    initializedSlots: new Uint8Array(buf.subarray(36, 44)),
    maxPrefabSlots: buf.readUInt8(44),
    maxCurvePoints: buf.readUInt8(45),
    switchSequence: buf.readBigUInt64LE(48)
  };
}
function isSlotInitialized(initializedSlots, curveType, slot, maxSlots) {
  const bitIndex = curveType * maxSlots + slot;
  const byteIndex = Math.floor(bitIndex / 8);
  const bitOffset = bitIndex % 8;
  return (initializedSlots[byteIndex] & 1 << bitOffset) !== 0;
}
function decodeFeeConfig(data) {
  requireLen(data, FEE_CONFIG_SIZE, "FeeConfig");
  const buf = Buffer.from(data);
  return {
    initialized: buf.readUInt8(0) !== 0,
    feePpm: buf.readUInt32LE(1),
    bump: buf.readUInt8(5),
    // skip 2 bytes padding
    feeAdmin: new (0, _web3js.PublicKey)(buf.subarray(8, 40)),
    feeRecipient: new (0, _web3js.PublicKey)(buf.subarray(40, 72))
  };
}
function getAssetFeeOverrides(data) {
  if (data.length < FEE_CONFIG_SIZE + ASSET_FEE_PAIR_COUNT_SIZE) return [];
  const buf = Buffer.from(data);
  const count = Math.min(buf.readUInt16LE(FEE_CONFIG_SIZE), MAX_ASSET_FEES);
  const entriesStart = FEE_CONFIG_SIZE + ASSET_FEE_PAIR_COUNT_SIZE;
  const out = [];
  for (let i = 0; i < count; i++) {
    const off = entriesStart + i * ASSET_FEE_ENTRY_SIZE;
    if (off + ASSET_FEE_ENTRY_SIZE > buf.length) break;
    out.push({
      mintA: new (0, _web3js.PublicKey)(buf.subarray(off, off + 32)),
      mintB: new (0, _web3js.PublicKey)(buf.subarray(off + 32, off + 64)),
      feePpm: buf.readUInt32LE(off + 64)
    });
  }
  return out;
}
function findAssetFeeOverride(data, mintA, mintB) {
  const [lo, hi] = Buffer.compare(mintA.toBuffer(), mintB.toBuffer()) <= 0 ? [mintA, mintB] : [mintB, mintA];
  for (const e of getAssetFeeOverrides(data)) {
    if (e.mintA.equals(lo) && e.mintB.equals(hi)) return e.feePpm;
  }
  return null;
}
function resolveFeePpm(feeConfigData, hasPoolFee, mintX, mintY) {
  const { feePpm } = decodeFeeConfig(feeConfigData);
  if (hasPoolFee) return { feePpm, source: "pool" };
  const pairPpm = findAssetFeeOverride(feeConfigData, mintX, mintY);
  return pairPpm !== null ? { feePpm: pairPpm, source: "pair" } : { feePpm, source: "global" };
}
function applyProtocolFee(amountIn, feePpm) {
  const net = amountIn * (1000000n - BigInt(feePpm)) / 1000000n;
  return { net, fee: amountIn - net };
}
var SPREAD_TRIGGER_LEN = 72;
function decodeSpreadConfig(data) {
  requireLen(data, 72, "SpreadConfig header");
  const buf = Buffer.from(data);
  const initialized = buf.readUInt8(0) !== 0;
  const bump = buf.readUInt8(1);
  const numTriggers = buf.readUInt8(2);
  const admin = new (0, _web3js.PublicKey)(buf.subarray(8, 40));
  const config = new (0, _web3js.PublicKey)(buf.subarray(40, 72));
  const triggers = [];
  const triggersStart = 72;
  requireLen(
    data,
    triggersStart + numTriggers * SPREAD_TRIGGER_LEN,
    `SpreadConfig with ${numTriggers} triggers`
  );
  for (let i = 0; i < numTriggers; i++) {
    const off = triggersStart + i * SPREAD_TRIGGER_LEN;
    const account = new (0, _web3js.PublicKey)(buf.subarray(off, off + 32));
    const programId = new (0, _web3js.PublicKey)(buf.subarray(off + 32, off + 64));
    const spreadBps = buf.readUInt16LE(off + 64);
    triggers.push({ account, programId, spreadBps });
  }
  return { initialized, bump, numTriggers, admin, config, triggers };
}
function decodeCurveUpdates(data) {
  requireLen(data, CURVE_UPDATES_SIZE, "CurveUpdates");
  const buf = Buffer.from(data);
  const authority = new (0, _web3js.PublicKey)(buf.subarray(0, 32));
  const curveMeta = new (0, _web3js.PublicKey)(buf.subarray(32, 64));
  const sequence = buf.readBigUInt64LE(64);
  const numOps = buf.readUInt8(72);
  const opsStart = 74;
  const ops = [];
  for (let i = 0; i < numOps && i < MAX_CURVE_UPDATE_OPS; i++) {
    const o = opsStart + i * CURVE_UPDATE_OP_SIZE;
    ops.push({
      curveType: checkEnum(buf.readUInt8(o), CURVE_TYPE_VALUES, "CurveType"),
      opKind: checkEnum(
        buf.readUInt8(o + 1),
        CURVE_UPDATE_OP_KIND_VALUES,
        "CurveUpdateOpKind"
      ),
      pointIndex: buf.readUInt8(o + 2),
      interpolation: checkEnum(
        buf.readUInt8(o + 3),
        INTERPOLATION_VALUES,
        "Interpolation"
      ),
      amountIn: buf.readBigUInt64LE(o + 4),
      priceFactorQ32: buf.readBigUInt64LE(o + 12),
      params: new Uint8Array(buf.subarray(o + 20, o + 24)),
      targetSlot: buf.readUInt8(o + 24),
      expectedXIn: buf.readBigUInt64LE(o + 25)
    });
  }
  return { authority, curveMeta, sequence, numOps, ops };
}
function decodeCurveSide(data, curveType, slot, maxSlots, maxPoints) {
  const sideSize = CURVE_SIDE_HEADER + maxPoints * CURVE_POINT_LEN;
  const offset = 32 + (curveType * maxSlots + slot) * sideSize;
  requireLen(data, offset + sideSize, `CurvePrefabs (curveType=${curveType}, slot=${slot})`);
  const buf = Buffer.from(data);
  const numPoints = buf.readUInt8(offset);
  const defaultInterpolation = checkEnum(
    buf.readUInt8(offset + 1),
    INTERPOLATION_VALUES,
    "Interpolation"
  );
  const xMode = checkEnum(buf.readUInt8(offset + 2), CURVE_X_MODE_VALUES, "CurveXMode");
  const riskMode = checkEnum(buf.readUInt8(offset + 3), RISK_MODE_VALUES, "RiskMode");
  const points = [];
  const pointsStart = offset + CURVE_SIDE_HEADER;
  const n = Math.min(numPoints, maxPoints);
  for (let i = 0; i < n; i++) {
    const pOff = pointsStart + i * CURVE_POINT_LEN;
    points.push({
      amountIn: buf.readBigUInt64LE(pOff),
      priceFactorQ32: buf.readBigUInt64LE(pOff + 8),
      interpolation: checkEnum(
        buf.readUInt8(pOff + 16),
        INTERPOLATION_VALUES,
        "Interpolation"
      ),
      params: new Uint8Array(buf.subarray(pOff + 17, pOff + 21))
    });
  }
  return { numPoints, defaultInterpolation, xMode, riskMode, points };
}
function decodeActiveCurves(prefabsData, meta) {
  const { maxPrefabSlots, maxCurvePoints } = meta;
  return {
    priceBid: decodeCurveSide(
      prefabsData,
      0 /* PriceBid */,
      meta.activePriceBidSlot,
      maxPrefabSlots,
      maxCurvePoints
    ),
    priceAsk: decodeCurveSide(
      prefabsData,
      1 /* PriceAsk */,
      meta.activePriceAskSlot,
      maxPrefabSlots,
      maxCurvePoints
    ),
    riskBid: decodeCurveSide(
      prefabsData,
      2 /* RiskBid */,
      meta.activeRiskBidSlot,
      maxPrefabSlots,
      maxCurvePoints
    ),
    riskAsk: decodeCurveSide(
      prefabsData,
      3 /* RiskAsk */,
      meta.activeRiskAskSlot,
      maxPrefabSlots,
      maxCurvePoints
    )
  };
}

// src/instructions/initialize.ts






function buildInitialize(payer, params, programId = HADRON_PROGRAM_ID) {
  const seed = params.seed;
  if (seed === void 0) throw new Error("seed is required \u2014 use Hadron.initialize() to auto-generate one");
  const tokenProgramX = _nullishCoalesce(params.tokenProgramX, () => ( _spltoken.TOKEN_PROGRAM_ID));
  const tokenProgramY = _nullishCoalesce(params.tokenProgramY, () => ( _spltoken.TOKEN_PROGRAM_ID));
  const creator = _nullishCoalesce(params.creator, () => ( params.authority));
  const lpSelfRotate = _nullishCoalesce(params.lpSelfRotate, () => ( false));
  const hasCustomSlots = params.maxPrefabSlots !== void 0 || params.maxCurvePoints !== void 0;
  const hasClamp = params.combinedFactorMaxQ32 !== void 0;
  if (hasClamp && !hasCustomSlots) {
    throw new Error(
      "buildInitialize: combinedFactorMaxQ32 requires maxPrefabSlots + maxCurvePoints (program rejects other shapes)"
    );
  }
  const maxPrefabSlots = _nullishCoalesce(params.maxPrefabSlots, () => ( DEFAULT_MAX_PREFAB_SLOTS));
  const maxCurvePoints = _nullishCoalesce(params.maxCurvePoints, () => ( DEFAULT_MAX_CURVE_POINTS));
  const addrs = derivePoolAddresses(
    creator,
    seed,
    params.mintX,
    params.mintY,
    tokenProgramX,
    tokenProgramY,
    programId
  );
  const extrasLen = hasClamp ? 10 : hasCustomSlots ? 2 : 0;
  const data = Buffer.alloc(1 + 8 + 32 + 32 + 32 + 8 + 1 + extrasLen);
  let offset = 0;
  data.writeUInt8(Discriminator.Initialize, offset);
  offset += 1;
  data.writeBigUInt64LE(seed, offset);
  offset += 8;
  params.mintX.toBuffer().copy(data, offset);
  offset += 32;
  params.mintY.toBuffer().copy(data, offset);
  offset += 32;
  params.authority.toBuffer().copy(data, offset);
  offset += 32;
  data.writeBigUInt64LE(params.initialMidpriceQ32, offset);
  offset += 8;
  data.writeUInt8(lpSelfRotate ? 1 : 0, offset);
  offset += 1;
  if (hasCustomSlots || hasClamp) {
    data.writeUInt8(maxPrefabSlots, offset);
    offset += 1;
    data.writeUInt8(maxCurvePoints, offset);
    offset += 1;
  }
  if (hasClamp) {
    data.writeBigUInt64LE(params.combinedFactorMaxQ32, offset);
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: creator, isSigner: true, isWritable: false },
      { pubkey: addrs.config, isSigner: false, isWritable: true },
      { pubkey: params.mintX, isSigner: false, isWritable: false },
      { pubkey: params.mintY, isSigner: false, isWritable: false },
      { pubkey: addrs.vaultX, isSigner: false, isWritable: true },
      { pubkey: addrs.vaultY, isSigner: false, isWritable: true },
      { pubkey: tokenProgramX, isSigner: false, isWritable: false },
      { pubkey: tokenProgramY, isSigner: false, isWritable: false },
      {
        pubkey: _spltoken.ASSOCIATED_TOKEN_PROGRAM_ID,
        isSigner: false,
        isWritable: false
      },
      { pubkey: _web3js.SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: addrs.midpriceOracle, isSigner: false, isWritable: true },
      { pubkey: addrs.curveMeta, isSigner: false, isWritable: true },
      { pubkey: addrs.curvePrefabs, isSigner: false, isWritable: true },
      { pubkey: addrs.curveUpdates, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildAllocateCurvePrefabs(payer, params, programId = HADRON_PROGRAM_ID) {
  const maxPrefabSlots = _nullishCoalesce(params.maxPrefabSlots, () => ( DEFAULT_MAX_PREFAB_SLOTS));
  const maxCurvePoints = _nullishCoalesce(params.maxCurvePoints, () => ( DEFAULT_MAX_CURVE_POINTS));
  const data = Buffer.alloc(1 + 8 + 32 + 32 + 1 + 1);
  let offset = 0;
  data.writeUInt8(Discriminator.AllocateCurvePrefabs, offset);
  offset += 1;
  data.writeBigUInt64LE(params.seed, offset);
  offset += 8;
  params.mintX.toBuffer().copy(data, offset);
  offset += 32;
  params.mintY.toBuffer().copy(data, offset);
  offset += 32;
  data.writeUInt8(maxPrefabSlots, offset);
  offset += 1;
  data.writeUInt8(maxCurvePoints, offset);
  const [curvePrefabsPda] = _web3js.PublicKey.findProgramAddressSync(
    [
      Buffer.from("hadron-curve-prefabs"),
      params.authority.toBuffer(),
      (() => {
        const b = Buffer.alloc(8);
        b.writeBigUInt64LE(params.seed);
        return b;
      })(),
      params.mintX.toBuffer(),
      params.mintY.toBuffer()
    ],
    programId
  );
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: params.authority, isSigner: true, isWritable: false },
      { pubkey: curvePrefabsPda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SystemProgram.programId, isSigner: false, isWritable: false }
    ],
    data
  });
}

// src/instructions/deposit.ts


function buildDeposit(user, configPda, midpriceOracle, mintX, mintY, tokenProgramX, tokenProgramY, params, programId = HADRON_PROGRAM_ID, needsCheckedTransferX = false, needsCheckedTransferY = false) {
  const data = Buffer.alloc(1 + 8 + 8 + 8);
  let offset = 0;
  data.writeUInt8(Discriminator.Deposit, offset);
  offset += 1;
  data.writeBigUInt64LE(params.amountX, offset);
  offset += 8;
  data.writeBigUInt64LE(params.amountY, offset);
  offset += 8;
  const exp = _nullishCoalesce(params.expiration, () => ( Math.floor(Date.now() / 1e3) + 3600));
  data.writeBigInt64LE(BigInt(exp), offset);
  const userX = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintX, user, true, tokenProgramX);
  const userY = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintY, user, true, tokenProgramY);
  const vaultX = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintX, configPda, true, tokenProgramX);
  const vaultY = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintY, configPda, true, tokenProgramY);
  const keys = [
    { pubkey: user, isSigner: true, isWritable: false },
    { pubkey: vaultX, isSigner: false, isWritable: true },
    { pubkey: vaultY, isSigner: false, isWritable: true },
    { pubkey: userX, isSigner: false, isWritable: true },
    { pubkey: userY, isSigner: false, isWritable: true },
    { pubkey: configPda, isSigner: false, isWritable: true },
    { pubkey: midpriceOracle, isSigner: false, isWritable: false },
    { pubkey: tokenProgramX, isSigner: false, isWritable: false },
    { pubkey: tokenProgramY, isSigner: false, isWritable: false }
  ];
  if (needsCheckedTransferX) {
    keys.push({ pubkey: mintX, isSigner: false, isWritable: false });
  }
  if (needsCheckedTransferY) {
    keys.push({ pubkey: mintY, isSigner: false, isWritable: false });
  }
  return new (0, _web3js.TransactionInstruction)({ programId, keys, data });
}

// src/instructions/withdraw.ts


function buildWithdraw(user, configPda, midpriceOracle, mintX, mintY, tokenProgramX, tokenProgramY, params, programId = HADRON_PROGRAM_ID, needsCheckedTransferX = false, needsCheckedTransferY = false) {
  const data = Buffer.alloc(1 + 8 + 8 + 8);
  let offset = 0;
  data.writeUInt8(Discriminator.Withdraw, offset);
  offset += 1;
  data.writeBigUInt64LE(params.amountX, offset);
  offset += 8;
  data.writeBigUInt64LE(params.amountY, offset);
  offset += 8;
  const exp = _nullishCoalesce(params.expiration, () => ( Math.floor(Date.now() / 1e3) + 3600));
  data.writeBigInt64LE(BigInt(exp), offset);
  const userX = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintX, user, true, tokenProgramX);
  const userY = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintY, user, true, tokenProgramY);
  const vaultX = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintX, configPda, true, tokenProgramX);
  const vaultY = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintY, configPda, true, tokenProgramY);
  const keys = [
    { pubkey: user, isSigner: true, isWritable: false },
    { pubkey: vaultX, isSigner: false, isWritable: true },
    { pubkey: vaultY, isSigner: false, isWritable: true },
    { pubkey: userX, isSigner: false, isWritable: true },
    { pubkey: userY, isSigner: false, isWritable: true },
    { pubkey: configPda, isSigner: false, isWritable: true },
    { pubkey: midpriceOracle, isSigner: false, isWritable: false },
    { pubkey: tokenProgramX, isSigner: false, isWritable: false },
    { pubkey: tokenProgramY, isSigner: false, isWritable: false }
  ];
  if (needsCheckedTransferX) {
    keys.push({ pubkey: mintX, isSigner: false, isWritable: false });
  }
  if (needsCheckedTransferY) {
    keys.push({ pubkey: mintY, isSigner: false, isWritable: false });
  }
  return new (0, _web3js.TransactionInstruction)({ programId, keys, data });
}

// src/instructions/swap.ts






function buildSwapExactIn(user, poolAddresses, authority, mintX, mintY, tokenProgramX, tokenProgramY, params, programId = HADRON_PROGRAM_ID, spreadConfigInitialized = false, feeConfigOverride, needsCheckedTransferX = false, needsCheckedTransferY = false) {
  const data = Buffer.alloc(1 + 1 + 8 + 8 + 8);
  let offset = 0;
  data.writeUInt8(Discriminator.SwapExactIn, offset);
  offset += 1;
  data.writeUInt8(params.isX ? 1 : 0, offset);
  offset += 1;
  data.writeBigUInt64LE(params.amountIn, offset);
  offset += 8;
  data.writeBigUInt64LE(params.minOut, offset);
  offset += 8;
  const exp = _nullishCoalesce(params.expiration, () => ( Math.floor(Date.now() / 1e3) + 3600));
  data.writeBigInt64LE(BigInt(exp), offset);
  const userX = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintX, user, true, tokenProgramX);
  const userY = _spltoken.getAssociatedTokenAddressSync.call(void 0, mintY, user, true, tokenProgramY);
  const [userSource, vaultSource, vaultDest, userDest] = params.isX ? [userX, poolAddresses.vaultX, poolAddresses.vaultY, userY] : [userY, poolAddresses.vaultY, poolAddresses.vaultX, userX];
  const feeConfigPda = _nullishCoalesce(feeConfigOverride, () => ( getFeeConfigAddress(programId)[0]));
  const inputMint = params.isX ? mintX : mintY;
  const inputMintProgram = params.isX ? tokenProgramX : tokenProgramY;
  if (!params.feeRecipient) {
    throw new Error(
      "buildSwapExactIn: params.feeRecipient is required. Either pass it explicitly, or call Hadron.swap() which reads the recipient from the pool's FeeConfig."
    );
  }
  const feeRecipientAta = _spltoken.getAssociatedTokenAddressSync.call(void 0, 
    inputMint,
    params.feeRecipient,
    true,
    inputMintProgram
  );
  const keys = [
    { pubkey: tokenProgramX, isSigner: false, isWritable: false },
    { pubkey: tokenProgramY, isSigner: false, isWritable: false },
    { pubkey: poolAddresses.config, isSigner: false, isWritable: true },
    {
      pubkey: poolAddresses.midpriceOracle,
      isSigner: false,
      isWritable: false
    },
    { pubkey: poolAddresses.curveMeta, isSigner: false, isWritable: false },
    {
      pubkey: poolAddresses.curvePrefabs,
      isSigner: false,
      isWritable: true
    },
    { pubkey: authority, isSigner: false, isWritable: false },
    // unused slot; program destructures as _authority
    { pubkey: user, isSigner: true, isWritable: false },
    { pubkey: userSource, isSigner: false, isWritable: true },
    { pubkey: vaultSource, isSigner: false, isWritable: true },
    { pubkey: vaultDest, isSigner: false, isWritable: true },
    { pubkey: userDest, isSigner: false, isWritable: true },
    { pubkey: feeConfigPda, isSigner: false, isWritable: false },
    { pubkey: feeRecipientAta, isSigner: false, isWritable: true },
    { pubkey: _web3js.SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
    {
      pubkey: poolAddresses.curveUpdates,
      isSigner: false,
      isWritable: true
    }
  ];
  if (needsCheckedTransferX) {
    keys.push({ pubkey: mintX, isSigner: false, isWritable: false });
  }
  if (needsCheckedTransferY) {
    keys.push({ pubkey: mintY, isSigner: false, isWritable: false });
  }
  if (spreadConfigInitialized) {
    const [spreadConfigPda] = getSpreadConfigAddress(
      poolAddresses.config,
      programId
    );
    keys.push(
      { pubkey: spreadConfigPda, isSigner: false, isWritable: false },
      { pubkey: _web3js.SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false }
    );
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys,
    data
  });
}

// src/instructions/setCurve.ts


// src/helpers/curveValidation.ts
var Q16 = 1n << 16n;
var Q8 = 1n << 8n;
function formatIssue(i) {
  if (i.kind === "segment") return `segment ${i.index}\u2192${i.index + 1}: ${i.code}`;
  if (i.kind === "point") return `point ${i.index}: ${i.code}`;
  return `curve: ${i.code} (point ${i.index})`;
}
var CurveRejectedError = class extends Error {
  constructor(curve, issues, points) {
    super(
      `The ${curve} curve would be rejected on-chain: ` + issues.map((i) => points ? explainIssue(i, points) : formatIssue(i)).join("; ") + `. Pass skipValidation: true to build it anyway.`
    );
    this.name = "CurveRejectedError";
    this.issues = issues;
  }
};
var SHAPE_NAME = ["Step", "Linear", "MarginalStep", "Hyperbolic", "Quadratic", "Cubic"];
var factorStr = (f) => (Number(f) / 2 ** 32).toFixed(6);
var feeBps = (f) => ((1 - Number(f) / 2 ** 32) * 1e4).toFixed(1);
function explainIssue(i, points) {
  const head = formatIssue(i);
  const p = points[i.index];
  const q = points[i.index + 1];
  switch (i.code) {
    case "ConcavityViolation": {
      if (!p || !q) return head;
      if (q.f > p.f) return `${head}: the factor rises from ${factorStr(p.f)} to ${factorStr(q.f)} (the fee falls); price factors must be non-increasing in x`;
      if (p.interp === 0) return `${head}: a Step segment cannot lower the factor (output would jump backwards at x=${q.x}); use Linear or MarginalStep, or keep both factors equal`;
      const minF = minLegalNextFactor(p.x, p.f, q.x, p.interp, p.params);
      if (minF == null) return `${head}: the ${_nullishCoalesce(SHAPE_NAME[p.interp], () => ( p.interp))} shape params are outside the range the chain accepts for this segment`;
      return `${head}: the factor drops too fast for a ${_nullishCoalesce(SHAPE_NAME[p.interp], () => ( p.interp))} segment; at x=${q.x} it must be at least ${factorStr(minF)} (fee \u2264 ${feeBps(minF)} bps)`;
    }
    case "PriceFactorTooLarge":
      return `${head}: factor ${p ? factorStr(p.f) : "?"} is above 1.0; a price curve cannot price better than mid`;
    case "CurvePointsNotSorted":
      return `${head}: x=${_optionalChain([p, 'optionalAccess', _ => _.x])} is below the previous point's x=${_optionalChain([points, 'access', _2 => _2[i.index - 1], 'optionalAccess', _3 => _3.x])}`;
    case "CurveDuplicateX":
      return `${head}: x=${_optionalChain([p, 'optionalAccess', _4 => _4.x])} repeats the previous point's x`;
    case "PriceCurveXOutOfRange":
    case "RiskCurveXOutOfRange":
      return `${head}: x=${_optionalChain([p, 'optionalAccess', _5 => _5.x])} is above 1.0 (Q32 ${Q32_ONE}) on a percent axis`;
    case "RiskCurveMonotonicityViolation":
      return i.kind === "curve" ? `${head}: the factor sequence changes direction at point ${i.index}; an inventory curve must be all non-increasing or all non-decreasing` : `${head}: the ${_nullishCoalesce(SHAPE_NAME[_nullishCoalesce(_optionalChain([p, 'optionalAccess', _6 => _6.interp]), () => ( 1))], () => ( _optionalChain([p, 'optionalAccess', _7 => _7.interp])))} shape params make this segment non-monotonic`;
    case "IntegratedRiskUnsupportedInterpolation":
      return `${head}: Integrated inventory curves support Step and Linear only`;
    case "InvalidRiskMode":
      return `${head}: Integrated inventory curves must use the absolute (vault balance) x axis`;
    case "CurveTooManyPoints":
      return `${head}: ${points.length} points exceed this pool's curve slot capacity`;
    case "InvalidInstructionData":
      return `${head}: interpolation byte ${_optionalChain([p, 'optionalAccess', _8 => _8.interp])} is not a known shape (0..=5)`;
    default:
      return head;
  }
}
function i16(b, o) {
  const v = b[o] | b[o + 1] << 8;
  return BigInt(v >= 32768 ? v - 65536 : v);
}
function i32(b) {
  return BigInt(b[0] | b[1] << 8 | b[2] << 16 | b[3] << 24 | 0);
}
function u32(b) {
  return BigInt((b[0] | b[1] << 8 | b[2] << 16 | b[3] << 24) >>> 0);
}
function ceilDiv(a, b) {
  return (a + b - 1n) / b;
}
var validInterp = (i) => Number.isInteger(i) && i >= 0 && i <= 5;
function validatePriceSegment(x0, f0, x1, f1, interp, params) {
  if (f0 < f1) return "ConcavityViolation";
  if (f0 === f1) return null;
  if (x1 <= x0) return null;
  const dx = x1 - x0;
  const df = f0 - f1;
  switch (interp) {
    case 0:
      return "ConcavityViolation";
    case 2:
      return x1 * f1 < x0 * f0 ? "ConcavityViolation" : null;
    case 1:
      return f1 * dx < x1 * df ? "ConcavityViolation" : null;
    case 4: {
      const k = i32(params);
      if (k >= 0n) {
        if (k > Q16) return "ConcavityViolation";
      } else {
        const maxAbs = dx * Q16 / (2n * dx + x0);
        if (-k > maxAbs) return "ConcavityViolation";
      }
      const ePrime = Q16 + k;
      return f1 * dx * Q16 < x1 * df * ePrime ? "ConcavityViolation" : null;
    }
    case 3: {
      const k = u32(params);
      if (k === 0n) return "ConcavityViolation";
      const minK = (x0 * Q16 + x1 - 1n) / x1;
      if (k < minK) return "ConcavityViolation";
      return f1 * dx * Q16 < x1 * df * k ? "ConcavityViolation" : null;
    }
    case 5: {
      const a = i16(params, 0);
      const b = i16(params, 2);
      if (a < 0n || a + 3n * b < 0n || a + b > Q8) return "ConcavityViolation";
      const ePrime = Q8 + a + 2n * b;
      if (ePrime > 0n && f1 * dx * Q8 < x1 * df * ePrime) return "ConcavityViolation";
      return null;
    }
    default:
      return "InvalidInstructionData";
  }
}
function minLegalNextFactor(x0, f0, x1, interp, params) {
  if (x1 <= x0) return null;
  const dx = x1 - x0;
  const solve = (E, K) => ceilDiv(x1 * f0 * E, dx * K + x1 * E);
  switch (interp) {
    case 0:
      return f0;
    case 2:
      return ceilDiv(x0 * f0, x1);
    case 1:
      return solve(1n, 1n);
    case 4: {
      const k = i32(params);
      if (k >= 0n) {
        if (k > Q16) return null;
      } else if (-k > dx * Q16 / (2n * dx + x0)) {
        return null;
      }
      return solve(Q16 + k, Q16);
    }
    case 3: {
      const k = u32(params);
      if (k === 0n || k < (x0 * Q16 + x1 - 1n) / x1) return null;
      return solve(k, Q16);
    }
    case 5: {
      const a = i16(params, 0);
      const b = i16(params, 2);
      if (a < 0n || a + 3n * b < 0n || a + b > Q8) return null;
      const ePrime = Q8 + a + 2n * b;
      if (ePrime <= 0n) return 0n;
      return solve(ePrime, Q8);
    }
    default:
      return null;
  }
}
function validatePriceCurve(points, xMode, opts = {}) {
  const errs = [];
  if (opts.maxCurvePoints != null && points.length > opts.maxCurvePoints) {
    errs.push({ index: 0, code: "CurveTooManyPoints", kind: "curve" });
  }
  points.forEach((p, i) => {
    if (!validInterp(p.interp)) errs.push({ index: i, code: "InvalidInstructionData", kind: "point" });
    if (p.f > Q32_ONE) errs.push({ index: i, code: "PriceFactorTooLarge", kind: "point" });
    if (i > 0) {
      if (p.x < points[i - 1].x) errs.push({ index: i, code: "CurvePointsNotSorted", kind: "point" });
      else if (p.x === points[i - 1].x) errs.push({ index: i, code: "CurveDuplicateX", kind: "point" });
    }
    if (xMode === "percent" && p.x > Q32_ONE) errs.push({ index: i, code: "PriceCurveXOutOfRange", kind: "point" });
  });
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1];
    const b = points[i];
    if (!validInterp(a.interp) || a.f > Q32_ONE || b.f > Q32_ONE) continue;
    const e = validatePriceSegment(a.x, a.f, b.x, b.f, a.interp, a.params);
    if (e) errs.push({ index: i - 1, code: e, kind: "segment" });
  }
  return errs;
}
function validateRiskSegmentParams(interp, params) {
  switch (interp) {
    case 0:
    case 1:
    case 2:
    case 3:
      return null;
    case 4: {
      const k = i32(params);
      return (k < 0n ? -k : k) > Q16 ? "RiskCurveMonotonicityViolation" : null;
    }
    case 5: {
      const a = i16(params, 0);
      const b = i16(params, 2);
      if (a + b > Q8 || a + 2n * b < -Q8) return "RiskCurveMonotonicityViolation";
      if (b > 0n && a <= 0n && -a <= 3n * b) {
        if (a * a > 3n * b * (Q8 - a - b)) return "RiskCurveMonotonicityViolation";
      }
      return null;
    }
    default:
      return "InvalidInstructionData";
  }
}
function validateRiskCurve(points, xMode, integrated = false, opts = {}) {
  const errs = [];
  if (opts.maxCurvePoints != null && points.length > opts.maxCurvePoints) {
    errs.push({ index: 0, code: "CurveTooManyPoints", kind: "curve" });
  }
  if (integrated && xMode !== "absolute") {
    errs.push({ index: 0, code: "InvalidRiskMode", kind: "curve" });
  }
  points.forEach((p, i) => {
    if (!validInterp(p.interp)) errs.push({ index: i, code: "InvalidInstructionData", kind: "point" });
    if (i > 0) {
      if (p.x < points[i - 1].x) errs.push({ index: i, code: "CurvePointsNotSorted", kind: "point" });
      else if (p.x === points[i - 1].x) errs.push({ index: i, code: "CurveDuplicateX", kind: "point" });
    }
    if (xMode === "percent" && p.x > Q32_ONE) errs.push({ index: i, code: "RiskCurveXOutOfRange", kind: "point" });
    if (integrated && validInterp(p.interp) && p.interp !== 0 && p.interp !== 1) {
      errs.push({ index: i, code: "IntegratedRiskUnsupportedInterpolation", kind: "point" });
    }
    if (validInterp(p.interp)) {
      const e = validateRiskSegmentParams(p.interp, p.params);
      if (e) errs.push({ index: i, code: e, kind: "point" });
    }
  });
  let up = false;
  let down = false;
  for (let i = 1; i < points.length; i++) {
    const prev = points[i - 1].f;
    const cur = points[i].f;
    if (cur > prev) {
      if (down) {
        errs.push({ index: i, code: "RiskCurveMonotonicityViolation", kind: "curve" });
        break;
      }
      up = true;
    } else if (cur < prev) {
      if (up) {
        errs.push({ index: i, code: "RiskCurveMonotonicityViolation", kind: "curve" });
        break;
      }
      down = true;
    }
  }
  return errs;
}
function assertCurveAccepted(curve, issues, points) {
  if (issues.length > 0) throw new CurveRejectedError(curve, issues, points);
}

// src/instructions/setCurve.ts
function interpolationToByte(interp) {
  return interp;
}
function buildSetCurve(authority, curveMetaPda, curvePrefabsPda, params, programId = HADRON_PROGRAM_ID) {
  const slot = _nullishCoalesce(params.slot, () => ( 0));
  const xMode = _nullishCoalesce(params.xMode, () => ( 0)) /* Native */;
  if (!params.skipValidation) {
    const pts = params.points.map((p) => ({
      x: p.amountIn,
      f: toQ32(p.priceFactor),
      interp: _nullishCoalesce(p.interpolation, () => ( params.defaultInterpolation)),
      params: _nullishCoalesce(p.params, () => ( new Uint8Array(4)))
    }));
    assertCurveAccepted(
      "price",
      validatePriceCurve(pts, xMode === 1 /* Alternate */ ? "percent" : "native", { maxCurvePoints: params.maxCurvePoints }),
      pts
    );
  }
  const numPoints = params.points.length;
  const data = Buffer.alloc(1 + 1 + 1 + 1 + 1 + 1 + numPoints * POINT_DATA_SIZE);
  let offset = 0;
  data.writeUInt8(Discriminator.SetCurve, offset);
  offset += 1;
  data.writeUInt8(slot, offset);
  offset += 1;
  data.writeUInt8(params.side, offset);
  offset += 1;
  data.writeUInt8(interpolationToByte(params.defaultInterpolation), offset);
  offset += 1;
  data.writeUInt8(numPoints, offset);
  offset += 1;
  data.writeUInt8(xMode, offset);
  offset += 1;
  for (const p of params.points) {
    data.writeBigUInt64LE(p.amountIn, offset);
    offset += 8;
    data.writeBigUInt64LE(toQ32(p.priceFactor), offset);
    offset += 8;
    data.writeUInt8(
      interpolationToByte(_nullishCoalesce(p.interpolation, () => ( params.defaultInterpolation))),
      offset
    );
    offset += 1;
    const params_ = _nullishCoalesce(p.params, () => ( new Uint8Array(4)));
    Buffer.from(params_).copy(data, offset, 0, 4);
    offset += 4;
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveMetaPda, isSigner: false, isWritable: true },
      { pubkey: curvePrefabsPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildSetRiskCurve(authority, curveMetaPda, curvePrefabsPda, params, programId = HADRON_PROGRAM_ID) {
  const slot = _nullishCoalesce(params.slot, () => ( 0));
  const xMode = _nullishCoalesce(params.xMode, () => ( 0)) /* Native */;
  const riskMode = _nullishCoalesce(params.riskMode, () => ( 0)) /* Virtual */;
  if (!params.skipValidation) {
    const pts = params.points.map((p) => ({
      x: toQ32(p.pctBase),
      f: toQ32(p.priceFactor),
      interp: _nullishCoalesce(p.interpolation, () => ( params.defaultInterpolation)),
      params: _nullishCoalesce(p.params, () => ( new Uint8Array(4)))
    }));
    assertCurveAccepted(
      "inventory",
      validateRiskCurve(pts, xMode === 1 /* Alternate */ ? "absolute" : "percent", riskMode === 1 /* Integrated */, { maxCurvePoints: params.maxCurvePoints }),
      pts
    );
  }
  const numPoints = params.points.length;
  const data = Buffer.alloc(
    1 + 1 + 1 + 1 + 1 + 1 + 1 + numPoints * POINT_DATA_SIZE
  );
  let offset = 0;
  data.writeUInt8(Discriminator.SetRiskCurve, offset);
  offset += 1;
  data.writeUInt8(slot, offset);
  offset += 1;
  data.writeUInt8(params.side, offset);
  offset += 1;
  data.writeUInt8(interpolationToByte(params.defaultInterpolation), offset);
  offset += 1;
  data.writeUInt8(numPoints, offset);
  offset += 1;
  data.writeUInt8(xMode, offset);
  offset += 1;
  data.writeUInt8(riskMode, offset);
  offset += 1;
  for (const p of params.points) {
    data.writeBigUInt64LE(toQ32(p.pctBase), offset);
    offset += 8;
    data.writeBigUInt64LE(toQ32(p.priceFactor), offset);
    offset += 8;
    data.writeUInt8(
      interpolationToByte(_nullishCoalesce(p.interpolation, () => ( params.defaultInterpolation))),
      offset
    );
    offset += 1;
    const params_ = _nullishCoalesce(p.params, () => ( new Uint8Array(4)));
    Buffer.from(params_).copy(data, offset, 0, 4);
    offset += 4;
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveMetaPda, isSigner: false, isWritable: true },
      { pubkey: curvePrefabsPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildSetRiskCurveAbsolute(authority, curveMetaPda, curvePrefabsPda, params, programId = HADRON_PROGRAM_ID) {
  const slot = _nullishCoalesce(params.slot, () => ( 0));
  const riskMode = _nullishCoalesce(params.riskMode, () => ( 0)) /* Virtual */;
  if (!params.skipValidation) {
    const pts = params.points.map((p) => ({
      x: p.vaultBalance,
      f: toQ32(p.priceFactor),
      interp: _nullishCoalesce(p.interpolation, () => ( params.defaultInterpolation)),
      params: _nullishCoalesce(p.params, () => ( new Uint8Array(4)))
    }));
    assertCurveAccepted(
      "inventory",
      validateRiskCurve(pts, "absolute", riskMode === 1 /* Integrated */, { maxCurvePoints: params.maxCurvePoints }),
      pts
    );
  }
  const numPoints = params.points.length;
  const data = Buffer.alloc(
    1 + 1 + 1 + 1 + 1 + 1 + 1 + numPoints * POINT_DATA_SIZE
  );
  let offset = 0;
  data.writeUInt8(Discriminator.SetRiskCurve, offset);
  offset += 1;
  data.writeUInt8(slot, offset);
  offset += 1;
  data.writeUInt8(params.side, offset);
  offset += 1;
  data.writeUInt8(interpolationToByte(params.defaultInterpolation), offset);
  offset += 1;
  data.writeUInt8(numPoints, offset);
  offset += 1;
  data.writeUInt8(1 /* Alternate */, offset);
  offset += 1;
  data.writeUInt8(riskMode, offset);
  offset += 1;
  for (const p of params.points) {
    data.writeBigUInt64LE(p.vaultBalance, offset);
    offset += 8;
    data.writeBigUInt64LE(toQ32(p.priceFactor), offset);
    offset += 8;
    data.writeUInt8(
      interpolationToByte(_nullishCoalesce(p.interpolation, () => ( params.defaultInterpolation))),
      offset
    );
    offset += 1;
    const params_ = _nullishCoalesce(p.params, () => ( new Uint8Array(4)));
    Buffer.from(params_).copy(data, offset, 0, 4);
    offset += 4;
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveMetaPda, isSigner: false, isWritable: true },
      { pubkey: curvePrefabsPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildSetCurveBoth(authority, curveMetaPda, curvePrefabsPda, params, programId = HADRON_PROGRAM_ID) {
  const bidIx = buildSetCurve(authority, curveMetaPda, curvePrefabsPda, { ...params.bid, side: 0 /* Bid */ }, programId);
  const askIx = buildSetCurve(authority, curveMetaPda, curvePrefabsPda, { ...params.ask, side: 1 /* Ask */ }, programId);
  return [bidIx, askIx];
}
function buildSetRiskCurveBoth(authority, curveMetaPda, curvePrefabsPda, params, programId = HADRON_PROGRAM_ID) {
  const bidIx = buildSetRiskCurve(authority, curveMetaPda, curvePrefabsPda, { ...params.bid, side: 0 /* Bid */ }, programId);
  const askIx = buildSetRiskCurve(authority, curveMetaPda, curvePrefabsPda, { ...params.ask, side: 1 /* Ask */ }, programId);
  return [bidIx, askIx];
}
function buildSetRiskCurveAbsoluteBoth(authority, curveMetaPda, curvePrefabsPda, params, programId = HADRON_PROGRAM_ID) {
  const bidIx = buildSetRiskCurveAbsolute(authority, curveMetaPda, curvePrefabsPda, { ...params.bid, side: 0 /* Bid */ }, programId);
  const askIx = buildSetRiskCurveAbsolute(authority, curveMetaPda, curvePrefabsPda, { ...params.ask, side: 1 /* Ask */ }, programId);
  return [bidIx, askIx];
}

// src/instructions/oracle.ts




function buildUpdateMidprice(authority, midpriceOraclePda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 8 + 8);
  data.writeUInt8(Discriminator.UpdateMidprice, 0);
  data.writeBigUInt64LE(_nullishCoalesce(params.sequence, () => ( 0n)), 1);
  data.writeBigUInt64LE(params.midpriceQ32, 9);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: midpriceOraclePda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildUpdateBaseSpread(authority, midpriceOraclePda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 8);
  data.writeUInt8(Discriminator.UpdateBaseSpread, 0);
  data.writeBigUInt64LE(params.spreadFactorQ32, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: midpriceOraclePda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildUpdateMidpriceAndBaseSpread(authority, midpriceOraclePda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 8 + 8 + 8);
  data.writeUInt8(Discriminator.UpdateMidpriceAndBaseSpread, 0);
  data.writeBigUInt64LE(_nullishCoalesce(params.sequence, () => ( 0n)), 1);
  data.writeBigUInt64LE(params.midpriceQ32, 9);
  data.writeBigUInt64LE(params.spreadFactorQ32, 17);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: midpriceOraclePda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false }
    ],
    data
  });
}

// src/instructions/switchCurve.ts

function buildSwitchPriceCurve(authority, curveMetaPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 8 + 1 + 1);
  data.writeUInt8(Discriminator.SwitchPriceCurve, 0);
  data.writeBigUInt64LE(params.sequence, 1);
  data.writeUInt8(params.side, 9);
  data.writeUInt8(params.slot, 10);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveMetaPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildSwitchRiskCurve(authority, curveMetaPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 8 + 1 + 1);
  data.writeUInt8(Discriminator.SwitchRiskCurve, 0);
  data.writeBigUInt64LE(params.sequence, 1);
  data.writeUInt8(params.side, 9);
  data.writeUInt8(params.slot, 10);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveMetaPda, isSigner: false, isWritable: true }
    ],
    data
  });
}

// src/instructions/curveUpdates.ts

function buildSubmitCurveUpdates(authority, curveUpdatesPda, sequence, ops, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 8 + 1 + ops.length * CURVE_UPDATE_OP_SIZE);
  let offset = 0;
  data.writeUInt8(Discriminator.SubmitCurveUpdates, offset);
  offset += 1;
  data.writeBigUInt64LE(sequence, offset);
  offset += 8;
  data.writeUInt8(ops.length, offset);
  offset += 1;
  for (const op of ops) {
    data.writeUInt8(op.curveType, offset);
    offset += 1;
    data.writeUInt8(op.opKind, offset);
    offset += 1;
    data.writeUInt8(op.pointIndex, offset);
    offset += 1;
    data.writeUInt8(op.interpolation, offset);
    offset += 1;
    data.writeBigUInt64LE(op.amountIn, offset);
    offset += 8;
    data.writeBigUInt64LE(op.priceFactorQ32, offset);
    offset += 8;
    const params = _nullishCoalesce(op.params, () => ( new Uint8Array(4)));
    Buffer.from(params).copy(data, offset, 0, 4);
    offset += 4;
    data.writeUInt8(op.targetSlot, offset);
    offset += 1;
    data.writeBigUInt64LE(op.expectedXIn, offset);
    offset += 8;
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveUpdatesPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildApplyCurveUpdates(authority, curveMetaPda, curvePrefabsPda, curveUpdatesPda, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1);
  data.writeUInt8(Discriminator.ApplyCurveUpdates, 0);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveMetaPda, isSigner: false, isWritable: false },
      { pubkey: curvePrefabsPda, isSigner: false, isWritable: true },
      { pubkey: curveUpdatesPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildValidateCurveUpdates(authority, curveMetaPda, curvePrefabsPda, curveUpdatesPda, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.from([Discriminator.ValidateCurveUpdates]);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: curveMetaPda, isSigner: false, isWritable: false },
      { pubkey: curvePrefabsPda, isSigner: false, isWritable: true },
      { pubkey: curveUpdatesPda, isSigner: false, isWritable: true }
    ],
    data
  });
}

// src/instructions/authority.ts




function buildNominateAuthority(authority, configPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 32 + 8);
  let offset = 0;
  data.writeUInt8(Discriminator.NominateAuthority, offset);
  offset += 1;
  params.newAuthority.toBuffer().copy(data, offset);
  offset += 32;
  data.writeBigUInt64LE(params.expirySlot, offset);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildAcceptAuthority(newAuthority, configPda, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1);
  data.writeUInt8(Discriminator.AcceptAuthority, 0);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: newAuthority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildRotateQuotingAuthority(authority, configPda, oraclePda, curveMetaPda, curveUpdatesPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 32);
  data.writeUInt8(Discriminator.RotateQuotingAuthority, 0);
  params.newQuotingAuthority.toBuffer().copy(data, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: oraclePda, isSigner: false, isWritable: true },
      { pubkey: curveMetaPda, isSigner: false, isWritable: true },
      { pubkey: curveUpdatesPda, isSigner: false, isWritable: true }
    ],
    data
  });
}

// src/instructions/feeConfig.ts




function buildInitializeFeeConfig(payer, authority, params, programId = HADRON_PROGRAM_ID) {
  const [feeConfigPda] = getFeeConfigAddress(programId);
  const data = Buffer.alloc(1 + 4 + 32 + 32);
  let offset = 0;
  data.writeUInt8(Discriminator.InitializeFeeConfig, offset);
  offset += 1;
  data.writeUInt32LE(params.feePpm, offset);
  offset += 4;
  params.feeAdmin.toBuffer().copy(data, offset);
  offset += 32;
  params.feeRecipient.toBuffer().copy(data, offset);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: feeConfigPda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SystemProgram.programId, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildUpdateFeeConfig(feeAdmin, params, programId = HADRON_PROGRAM_ID) {
  const [feeConfigPda] = getFeeConfigAddress(programId);
  const data = Buffer.alloc(1 + 4 + 32);
  let offset = 0;
  data.writeUInt8(Discriminator.UpdateFeeConfig, offset);
  offset += 1;
  data.writeUInt32LE(params.feePpm !== null ? params.feePpm : 4294967295, offset);
  offset += 4;
  if (params.feeRecipient) {
    params.feeRecipient.toBuffer().copy(data, offset);
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: feeAdmin, isSigner: true, isWritable: false },
      { pubkey: feeConfigPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildRotateFeeAdmin(feeAdmin, params, programId = HADRON_PROGRAM_ID) {
  const [feeConfigPda] = getFeeConfigAddress(programId);
  const data = Buffer.alloc(1 + 32);
  data.writeUInt8(Discriminator.RotateFeeAdmin, 0);
  params.newFeeAdmin.toBuffer().copy(data, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: feeAdmin, isSigner: true, isWritable: false },
      { pubkey: feeConfigPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildInitializePoolFeeConfig(payer, feeAdmin, configPda, params, programId = HADRON_PROGRAM_ID) {
  const [poolFeeConfigPda] = getPoolFeeConfigAddress(configPda, programId);
  const [globalFeeConfigPda] = getFeeConfigAddress(programId);
  const data = Buffer.alloc(1 + 4 + 32 + 32);
  let offset = 0;
  data.writeUInt8(Discriminator.InitializePoolFeeConfig, offset);
  offset += 1;
  data.writeUInt32LE(params.feePpm, offset);
  offset += 4;
  params.feeAdmin.toBuffer().copy(data, offset);
  offset += 32;
  params.feeRecipient.toBuffer().copy(data, offset);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: feeAdmin, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: poolFeeConfigPda, isSigner: false, isWritable: true },
      { pubkey: globalFeeConfigPda, isSigner: false, isWritable: false },
      { pubkey: _web3js.SystemProgram.programId, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildSetAssetFee(payer, feeAdmin, params, programId = HADRON_PROGRAM_ID) {
  const [feeConfigPda] = getFeeConfigAddress(programId);
  const data = Buffer.alloc(1 + 32 + 32 + 4);
  let offset = 0;
  data.writeUInt8(Discriminator.SetAssetFee, offset);
  offset += 1;
  params.mintA.toBuffer().copy(data, offset);
  offset += 32;
  params.mintB.toBuffer().copy(data, offset);
  offset += 32;
  data.writeUInt32LE(params.feePpm, offset);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: feeAdmin, isSigner: true, isWritable: false },
      { pubkey: feeConfigPda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SystemProgram.programId, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildClosePoolFeeConfig(feeAdmin, configPda, programId = HADRON_PROGRAM_ID) {
  const [poolFeeConfigPda] = getPoolFeeConfigAddress(configPda, programId);
  const [globalFeeConfigPda] = getFeeConfigAddress(programId);
  const data = Buffer.from([Discriminator.ClosePoolFeeConfig]);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      // DIVERGENCE FROM CANONICAL FIXTURE: canonical marks fee_admin readonly,
      // but close_pda transfers the pool_fee_config rent here, which requires
      // writable. Mirrors the close_pool divergence in poolState.ts. Mollusk
      // is lenient about lamport writes to readonly accounts; LiteSVM and the
      // real runtime are not.
      { pubkey: feeAdmin, isSigner: true, isWritable: true },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: poolFeeConfigPda, isSigner: false, isWritable: true },
      { pubkey: globalFeeConfigPda, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildUpdatePoolFeeConfig(feeAdmin, configPda, params, programId = HADRON_PROGRAM_ID) {
  const [poolFeeConfigPda] = getPoolFeeConfigAddress(configPda, programId);
  const data = Buffer.alloc(1 + 4 + 32);
  let offset = 0;
  data.writeUInt8(Discriminator.UpdatePoolFeeConfig, offset);
  offset += 1;
  data.writeUInt32LE(params.feePpm !== null ? params.feePpm : 4294967295, offset);
  offset += 4;
  if (params.feeRecipient) {
    params.feeRecipient.toBuffer().copy(data, offset);
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: feeAdmin, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: poolFeeConfigPda, isSigner: false, isWritable: true }
    ],
    data
  });
}

// src/instructions/spreadConfig.ts




function buildInitializeSpreadConfig(payer, authority, configPda, params, programId = HADRON_PROGRAM_ID) {
  const [spreadConfigPda] = getSpreadConfigAddress(configPda, programId);
  const data = Buffer.alloc(1 + 32);
  data.writeUInt8(Discriminator.InitializeSpreadConfig, 0);
  params.admin.toBuffer().copy(data, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: spreadConfigPda, isSigner: false, isWritable: true },
      { pubkey: _web3js.SystemProgram.programId, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildUpdateSpreadConfig(admin, configPda, params, programId = HADRON_PROGRAM_ID) {
  const [spreadConfigPda] = getSpreadConfigAddress(configPda, programId);
  const numTriggers = params.triggers.length;
  const data = Buffer.alloc(1 + 1 + numTriggers * 66);
  let offset = 0;
  data.writeUInt8(Discriminator.UpdateSpreadConfig, offset);
  offset += 1;
  data.writeUInt8(numTriggers, offset);
  offset += 1;
  for (const t of params.triggers) {
    t.account.toBuffer().copy(data, offset);
    offset += 32;
    t.programId.toBuffer().copy(data, offset);
    offset += 32;
    data.writeUInt16LE(t.spreadBps, offset);
    offset += 2;
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: admin, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: spreadConfigPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildRemoveSpreadTriggers(admin, configPda, params, programId = HADRON_PROGRAM_ID) {
  const [spreadConfigPda] = getSpreadConfigAddress(configPda, programId);
  const numKeys = params.keys.length;
  const data = Buffer.alloc(1 + 1 + numKeys * 64);
  let offset = 0;
  data.writeUInt8(Discriminator.RemoveSpreadTriggers, offset);
  offset += 1;
  data.writeUInt8(numKeys, offset);
  offset += 1;
  for (const k of params.keys) {
    k.account.toBuffer().copy(data, offset);
    offset += 32;
    k.programId.toBuffer().copy(data, offset);
    offset += 32;
  }
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: admin, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: spreadConfigPda, isSigner: false, isWritable: true }
    ],
    data
  });
}

// src/instructions/poolState.ts



function buildSetPoolState(authority, configPda, curveMetaPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 1);
  data.writeUInt8(Discriminator.SetPoolState, 0);
  data.writeUInt8(params.newState, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: curveMetaPda, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildUpdateDeltaStaleness(authority, configPda, curveMetaPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 1);
  data.writeUInt8(Discriminator.UpdateDeltaStaleness, 0);
  data.writeUInt8(params.deltaStaleness, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: curveMetaPda, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildClosePool(authority, configPda, midpriceOraclePda, curveMetaPda, curvePrefabsPda, curveUpdatesPda, vaultX, vaultY, tokenProgramX, tokenProgramY, spreadConfigInitialized = false, withdrawAtaX, withdrawAtaY, programId = HADRON_PROGRAM_ID, mintX, mintY) {
  const data = Buffer.alloc(1);
  data.writeUInt8(Discriminator.ClosePool, 0);
  const keys = [
    // DIVERGENCE FROM CANONICAL FIXTURE: canonical builder marks authority
    // readonly, but close_token_account CPIs it as the writable destination
    // for vault lamports (token.rs::close_token_account). Solana's runtime
    // rejects readonly-signer → writable-CPI privilege escalation, so this
    // builder marks the authority writable. The canonical fixture diff
    // surfaces this as a flag mismatch — keep it; mollusk just happens to
    // be lenient about the same write that real-runtime/LiteSVM rejects.
    { pubkey: authority, isSigner: true, isWritable: true },
    { pubkey: configPda, isSigner: false, isWritable: true },
    { pubkey: midpriceOraclePda, isSigner: false, isWritable: true },
    { pubkey: curveMetaPda, isSigner: false, isWritable: true },
    { pubkey: curvePrefabsPda, isSigner: false, isWritable: true },
    { pubkey: curveUpdatesPda, isSigner: false, isWritable: true },
    { pubkey: vaultX, isSigner: false, isWritable: true },
    { pubkey: vaultY, isSigner: false, isWritable: true },
    { pubkey: tokenProgramX, isSigner: false, isWritable: false },
    { pubkey: tokenProgramY, isSigner: false, isWritable: false }
  ];
  if (mintX) {
    keys.push({ pubkey: mintX, isSigner: false, isWritable: false });
  }
  if (mintY) {
    keys.push({ pubkey: mintY, isSigner: false, isWritable: false });
  }
  if (spreadConfigInitialized) {
    const [spreadConfigPda] = getSpreadConfigAddress(configPda, programId);
    keys.push({
      pubkey: spreadConfigPda,
      isSigner: false,
      isWritable: true
    });
  }
  if (withdrawAtaX) {
    keys.push({ pubkey: withdrawAtaX, isSigner: false, isWritable: true });
  }
  if (withdrawAtaY) {
    keys.push({ pubkey: withdrawAtaY, isSigner: false, isWritable: true });
  }
  return new (0, _web3js.TransactionInstruction)({ programId, keys, data });
}

// src/instructions/governance.ts

function buildUpdateDrawdownGuard(authority, configPda, midpriceOraclePda, vaultX, vaultY, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 2 + 4);
  data.writeUInt8(Discriminator.UpdateDrawdownGuard, 0);
  data.writeUInt16LE(params.thresholdBps, 1);
  data.writeUInt32LE(params.rotationIntervalSlots, 3);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: midpriceOraclePda, isSigner: false, isWritable: false },
      { pubkey: vaultX, isSigner: false, isWritable: false },
      { pubkey: vaultY, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildPokeDrawdownGuard(authority, configPda, midpriceOraclePda, vaultX, vaultY, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.from([Discriminator.PokeDrawdownGuard]);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: midpriceOraclePda, isSigner: false, isWritable: false },
      { pubkey: vaultX, isSigner: false, isWritable: false },
      { pubkey: vaultY, isSigner: false, isWritable: false }
    ],
    data
  });
}
function buildUpdateFactorClamp(authority, configPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 8);
  data.writeUInt8(Discriminator.UpdateFactorClamp, 0);
  data.writeBigUInt64LE(params.combinedFactorMaxQ32, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: true },
      { pubkey: configPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildRotateDepositAuthority(signer, configPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 32);
  data.writeUInt8(Discriminator.RotateDepositAuthority, 0);
  params.newAuthority.toBuffer().copy(data, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: signer, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildRotateWithdrawAuthority(signer, configPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 32);
  data.writeUInt8(Discriminator.RotateWithdrawAuthority, 0);
  params.newAuthority.toBuffer().copy(data, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: signer, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildRotateSpreadAdmin(authority, configPda, params, programId = HADRON_PROGRAM_ID) {
  const [spreadConfigPda] = getSpreadConfigAddress(configPda, programId);
  const data = Buffer.alloc(1 + 32);
  data.writeUInt8(Discriminator.RotateSpreadAdmin, 0);
  params.newSpreadAdmin.toBuffer().copy(data, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: false },
      { pubkey: spreadConfigPda, isSigner: false, isWritable: true }
    ],
    data
  });
}
function buildRotatePauseAuthority(authority, configPda, params, programId = HADRON_PROGRAM_ID) {
  const data = Buffer.alloc(1 + 32);
  data.writeUInt8(Discriminator.RotatePauseAuthority, 0);
  params.newAuthority.toBuffer().copy(data, 1);
  return new (0, _web3js.TransactionInstruction)({
    programId,
    keys: [
      { pubkey: authority, isSigner: true, isWritable: false },
      { pubkey: configPda, isSigner: false, isWritable: true }
    ],
    data
  });
}

// src/hadron.ts
var Hadron = class _Hadron {
  constructor(connection, poolAddress, addresses, config, oracle, curveMeta, curvePrefabsData, programId = HADRON_PROGRAM_ID, feeConfig, feeConfigData) {
    this.connection = connection;
    this.poolAddress = poolAddress;
    this.addresses = addresses;
    this.config = config;
    this.oracle = oracle;
    this.curveMeta = curveMeta;
    this.curvePrefabsData = curvePrefabsData;
    this.feeConfig = feeConfig;
    this.feeConfigData = feeConfigData;
    this.programId = programId;
  }
  /** PDA of the fee-config account this pool uses (pool-specific or global). */
  get feeConfigPda() {
    return this.config.hasPoolFee ? getPoolFeeConfigAddress(this.poolAddress, this.programId)[0] : getFeeConfigAddress(this.programId)[0];
  }
  /**
   * The protocol fee rate a swap on THIS pool actually pays, and where it came
   * from — the pool-specific override, a per-mint-pair override, or the global
   * default. See {@link resolveFeePpm} for the precedence rules.
   *
   * Prefer this over reading `feeConfig.feePpm`, which is only the header rate
   * and ignores any pair override. Throws if the instance was built without
   * fee-config bytes (i.e. constructed directly rather than via `load()`).
   */
  resolveFee() {
    if (!this.feeConfigData) {
      throw new Error(
        "Hadron.resolveFee(): no fee-config bytes on this instance \u2014 load it with Hadron.load() or pass feeConfigData to the constructor"
      );
    }
    return resolveFeePpm(
      this.feeConfigData,
      this.config.hasPoolFee,
      this.config.mintX,
      this.config.mintY
    );
  }
  // ==========================================================================
  // Static Factory Methods
  // ==========================================================================
  /**
   * Load a Hadron instance from an existing pool address.
   * Fetches all required accounts from the chain.
   */
  static async load(connection, poolAddress) {
    const programId = HADRON_PROGRAM_ID;
    const configInfo = await connection.getAccountInfo(poolAddress);
    if (!configInfo) throw new Error("Config account not found");
    const config = decodeConfig(configInfo.data);
    const tokenProgramX = config.tokenProgramX;
    const tokenProgramY = config.tokenProgramY;
    const addresses = derivePoolAddresses(
      config.authority,
      config.seed,
      config.mintX,
      config.mintY,
      tokenProgramX,
      tokenProgramY,
      programId
    );
    addresses.curvePrefabs = config.curvePrefabs;
    const feeConfigPda = config.hasPoolFee ? getPoolFeeConfigAddress(poolAddress, programId)[0] : getFeeConfigAddress(programId)[0];
    const [oracleInfo, curveMetaInfo, curvePrefabsInfo, feeConfigInfo] = await Promise.all([
      connection.getAccountInfo(addresses.midpriceOracle),
      connection.getAccountInfo(addresses.curveMeta),
      connection.getAccountInfo(addresses.curvePrefabs),
      connection.getAccountInfo(feeConfigPda)
    ]);
    if (!oracleInfo) throw new Error("MidpriceOracle account not found");
    if (!curveMetaInfo) throw new Error("CurveMeta account not found");
    if (!curvePrefabsInfo) throw new Error("CurvePrefabs account not found");
    if (!feeConfigInfo)
      throw new Error(
        config.hasPoolFee ? "Pool-specific FeeConfig account not found (config.has_pool_fee=true)" : "Global FeeConfig account not found"
      );
    const oracle = decodeMidpriceOracle(oracleInfo.data);
    const curveMeta = decodeCurveMeta(curveMetaInfo.data);
    const feeConfig = decodeFeeConfig(feeConfigInfo.data);
    return new _Hadron(
      connection,
      poolAddress,
      addresses,
      config,
      oracle,
      curveMeta,
      curvePrefabsInfo.data,
      programId,
      feeConfig,
      feeConfigInfo.data
    );
  }
  /**
   * Load a Hadron instance from pool identity (seed + mints).
   */
  static async loadFromSeed(connection, seed, mintX, mintY) {
    const programId = HADRON_PROGRAM_ID;
    const [poolAddress] = getConfigAddress(seed, mintX, mintY, programId);
    return _Hadron.load(connection, poolAddress);
  }
  /**
   * Build all instructions needed to initialize a new pool.
   *
   * Returns the allocate + initialize + setPoolState instructions and the
   * derived pool address. The on-chain Initialize ix always writes state
   * Initialized (swaps live immediately), so a SetPoolState(RejectSwaps) ix is
   * appended by default — the pool stays swap-dark (deposits/withdraws still
   * work) until the authority flips it to Initialized. Pass
   * `initialPoolState: PoolState.Initialized` to skip it.
   *
   * The caller is responsible for grouping them into transactions and sending.
   * The allocate instruction(s) must be confirmed before the initialize
   * instruction; initialize + setPoolState can share a transaction (both are
   * signed by `authority`).
   *
   * After the pool is live on-chain, load it with `Hadron.load(connection, poolAddress)`.
   *
   * ```ts
   * const { instructions, poolAddress, seed } = Hadron.initialize(payer, {
   *   mintX,
   *   mintY,
   *   authority: payer,
   *   initialMidpriceQ32: Q32_ONE,
   * });
   * ```
   */
  static initialize(payer, params) {
    const programId = HADRON_PROGRAM_ID;
    const seed = _nullishCoalesce(params.seed, () => ( _Hadron.randomSeed()));
    const maxPrefabSlots = _nullishCoalesce(params.maxPrefabSlots, () => ( DEFAULT_MAX_PREFAB_SLOTS));
    const maxCurvePoints = _nullishCoalesce(params.maxCurvePoints, () => ( DEFAULT_MAX_CURVE_POINTS));
    const resolvedParams = { ...params, seed };
    const [poolAddress] = getConfigAddress(
      seed,
      params.mintX,
      params.mintY,
      programId
    );
    const allocateParams = {
      seed,
      mintX: params.mintX,
      mintY: params.mintY,
      maxPrefabSlots,
      maxCurvePoints,
      authority: params.authority
    };
    const instructions = [];
    const size = curvePrefabsSize(maxPrefabSlots, maxCurvePoints);
    const allocateCalls = size > 10240 ? 2 : 1;
    for (let i = 0; i < allocateCalls; i++) {
      instructions.push(buildAllocateCurvePrefabs(payer, allocateParams, programId));
    }
    instructions.push(buildInitialize(payer, resolvedParams, programId));
    const initialPoolState = _nullishCoalesce(params.initialPoolState, () => ( 3)) /* RejectSwaps */;
    if (initialPoolState !== 1 /* Initialized */) {
      const [curveMeta] = getCurveMetaAddress(
        seed,
        params.mintX,
        params.mintY,
        programId
      );
      instructions.push(
        buildSetPoolState(
          params.authority,
          poolAddress,
          curveMeta,
          { newState: initialPoolState },
          programId
        )
      );
    }
    return { instructions, poolAddress, seed };
  }
  /**
   * Build idempotent create-ATA instructions for the fee recipient for both mints.
   *
   * These are no-ops on-chain if the ATAs already exist. Include them alongside
   * initialize or first deposit to ensure swaps don't fail due to missing fee ATAs.
   */
  static buildFeeRecipientAtaIxs(payer, feeRecipient, mintX, mintY, tokenProgramX, tokenProgramY) {
    return buildFeeRecipientAtaIxs(
      payer,
      feeRecipient,
      mintX,
      mintY,
      _nullishCoalesce(tokenProgramX, () => ( _spltoken.TOKEN_PROGRAM_ID)),
      _nullishCoalesce(tokenProgramY, () => ( _spltoken.TOKEN_PROGRAM_ID))
    );
  }
  /** Generate a random u64 seed. */
  static randomSeed() {
    const buf = new Uint8Array(8);
    crypto.getRandomValues(buf);
    return new DataView(buf.buffer).getBigUint64(0, true);
  }
  // ==========================================================================
  // State Queries
  // ==========================================================================
  /** Re-fetch all account state from the chain. */
  async refetchStates() {
    const [configInfo, oracleInfo, curveMetaInfo, curvePrefabsInfo, feeConfigInfo] = await Promise.all([
      this.connection.getAccountInfo(this.poolAddress),
      this.connection.getAccountInfo(this.addresses.midpriceOracle),
      this.connection.getAccountInfo(this.addresses.curveMeta),
      this.connection.getAccountInfo(this.addresses.curvePrefabs),
      this.connection.getAccountInfo(this.feeConfigPda)
    ]);
    if (configInfo) this.config = decodeConfig(configInfo.data);
    if (oracleInfo) this.oracle = decodeMidpriceOracle(oracleInfo.data);
    if (curveMetaInfo) this.curveMeta = decodeCurveMeta(curveMetaInfo.data);
    if (curvePrefabsInfo) this.curvePrefabsData = curvePrefabsInfo.data;
    if (feeConfigInfo) {
      this.feeConfig = decodeFeeConfig(feeConfigInfo.data);
      this.feeConfigData = feeConfigInfo.data;
    }
  }
  /** Get midprice as a floating-point number. */
  getMidprice() {
    return fromQ32(this.oracle.midpriceQ32);
  }
  /** Get spread factor as a floating-point number (e.g. 0.9995 = 5 bps). */
  getSpreadFactor() {
    return fromQ32(this.oracle.spreadFactorQ32);
  }
  /** Get spread in basis points (e.g. 5 for a 0.9995 factor). */
  getSpreadBps() {
    return (1 - fromQ32(this.oracle.spreadFactorQ32)) * 1e4;
  }
  /** Get the currently active curve slot indices. */
  getActiveCurveSlots() {
    return {
      priceBid: this.curveMeta.activePriceBidSlot,
      priceAsk: this.curveMeta.activePriceAskSlot,
      riskBid: this.curveMeta.activeRiskBidSlot,
      riskAsk: this.curveMeta.activeRiskAskSlot
    };
  }
  /** Decode the currently active curves from prefabs data. */
  getActiveCurves() {
    return decodeActiveCurves(this.curvePrefabsData, this.curveMeta);
  }
  /** Decode a specific curve slot. */
  getCurveSlot(curveType, slot) {
    return decodeCurveSide(
      this.curvePrefabsData,
      curveType,
      slot,
      this.curveMeta.maxPrefabSlots,
      this.curveMeta.maxCurvePoints
    );
  }
  // ==========================================================================
  // Inventory / Spread Skew Helpers
  // ==========================================================================
  /**
   * Compute the current inventory balance as a percentage of vault value in base token (X).
   *
   * Returns a value between 0.0 (all quote) and 1.0 (all base). 0.5 means balanced.
   * Values above 0.5 indicate skew toward base; below 0.5 toward quote.
   *
   * @param vaultX Raw base token amount (atoms) in vault X.
   * @param vaultY Raw quote token amount (atoms) in vault Y.
   */
  getInventoryBalance(vaultX, vaultY) {
    const midprice = fromQ32(this.oracle.midpriceQ32);
    if (midprice === 0) return 0.5;
    const baseValue = Number(vaultX);
    const quoteInBase = Number(vaultY) / midprice;
    const total = baseValue + quoteInBase;
    if (total === 0) return 0.5;
    return baseValue / total;
  }
  /**
   * Get the current inventory risk factors from the active risk (inventory) curves
   * at the given vault balances.
   *
   * Returns the inventory balance and the bid/ask risk factors.
   * A factor of 1.0 means no inventory adjustment. Factors below 1.0 widen the spread
   * on that side (worse price for the trader), providing inventory rebalancing incentive.
   *
   * @throws if either risk curve uses `CurveXMode.Alternate` (absolute vault balance x-axis).
   *
   * @param vaultX Raw base token amount (atoms) in vault X.
   * @param vaultY Raw quote token amount (atoms) in vault Y.
   */
  getInventoryRiskFactors(vaultX, vaultY) {
    const curves = this.getActiveCurves();
    if (curves.riskBid.xMode === 1 /* Alternate */) {
      throw new Error(
        "risk bid curve uses Alternate (absolute) x_mode; only Native (percent) mode is supported"
      );
    }
    if (curves.riskAsk.xMode === 1 /* Alternate */) {
      throw new Error(
        "risk ask curve uses Alternate (absolute) x_mode; only Native (percent) mode is supported"
      );
    }
    const pctBase = this.getInventoryBalance(vaultX, vaultY);
    const pctQ32 = toQ32(pctBase);
    return {
      pctBase,
      riskFactorBid: evaluateRiskCurveAt(curves.riskBid, pctQ32),
      riskFactorAsk: evaluateRiskCurveAt(curves.riskAsk, pctQ32)
    };
  }
  // ==========================================================================
  // Instruction Builders (convenience wrappers)
  // ==========================================================================
  /** True when mint X uses Token-2022 (needs transfer_checked + mint account). */
  get needsCheckedTransferX() {
    return !this.config.tokenProgramX.equals(_spltoken.TOKEN_PROGRAM_ID);
  }
  /** True when mint Y uses Token-2022 (needs transfer_checked + mint account). */
  get needsCheckedTransferY() {
    return !this.config.tokenProgramY.equals(_spltoken.TOKEN_PROGRAM_ID);
  }
  /** Build deposit instruction. */
  deposit(user, params) {
    return buildDeposit(
      user,
      this.poolAddress,
      this.addresses.midpriceOracle,
      this.config.mintX,
      this.config.mintY,
      this.config.tokenProgramX,
      this.config.tokenProgramY,
      params,
      this.programId,
      this.needsCheckedTransferX,
      this.needsCheckedTransferY
    );
  }
  /** Build withdraw instruction. */
  withdraw(user, params) {
    return buildWithdraw(
      user,
      this.poolAddress,
      this.addresses.midpriceOracle,
      this.config.mintX,
      this.config.mintY,
      this.config.tokenProgramX,
      this.config.tokenProgramY,
      params,
      this.programId,
      this.needsCheckedTransferX,
      this.needsCheckedTransferY
    );
  }
  /**
   * Build swap instruction.
   *
   * `params.feeRecipient` is optional; when omitted, the recipient from the
   * pool's cached `FeeConfig` is used.
   */
  swap(user, params) {
    const feeConfigOverride = this.config.hasPoolFee ? getPoolFeeConfigAddress(this.poolAddress, this.programId)[0] : void 0;
    const defaultRecipient = _optionalChain([this, 'access', _9 => _9.feeConfig, 'optionalAccess', _10 => _10.feeRecipient]);
    if (!params.feeRecipient && !defaultRecipient) {
      throw new Error(
        "Hadron.swap: params.feeRecipient is required when this Hadron instance was constructed without a cached FeeConfig. Either pass params.feeRecipient explicitly or load the pool via Hadron.load()."
      );
    }
    const resolvedParams = {
      ...params,
      feeRecipient: _nullishCoalesce(params.feeRecipient, () => ( defaultRecipient))
    };
    return buildSwapExactIn(
      user,
      this.addresses,
      this.config.authority,
      this.config.mintX,
      this.config.mintY,
      this.config.tokenProgramX,
      this.config.tokenProgramY,
      resolvedParams,
      this.programId,
      this.config.spreadConfigInitialized,
      feeConfigOverride,
      this.needsCheckedTransferX,
      this.needsCheckedTransferY
    );
  }
  /** Build set curve instruction. */
  setCurve(authority, params) {
    return buildSetCurve(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      { maxCurvePoints: this.curveMeta.maxCurvePoints, ...params },
      this.programId
    );
  }
  /** Build set risk curve instruction (percent-based x-axis). */
  setRiskCurve(authority, params) {
    return buildSetRiskCurve(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      { maxCurvePoints: this.curveMeta.maxCurvePoints, ...params },
      this.programId
    );
  }
  /** Build set risk curve instruction (absolute x-axis). */
  setRiskCurveAbsolute(authority, params) {
    return buildSetRiskCurveAbsolute(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      { maxCurvePoints: this.curveMeta.maxCurvePoints, ...params },
      this.programId
    );
  }
  /** Build set curve instructions for both bid and ask. Returns [bidIx, askIx]. */
  setCurveBoth(authority, params) {
    return buildSetCurveBoth(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      params,
      this.programId
    );
  }
  /** Build set risk curve instructions for both bid and ask. Returns [bidIx, askIx]. */
  setRiskCurveBoth(authority, params) {
    return buildSetRiskCurveBoth(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      params,
      this.programId
    );
  }
  /** Build set risk curve (absolute x-axis) instructions for both bid and ask. Returns [bidIx, askIx]. */
  setRiskCurveAbsoluteBoth(authority, params) {
    return buildSetRiskCurveAbsoluteBoth(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      params,
      this.programId
    );
  }
  /** Build update midprice instruction. */
  updateMidprice(authority, params) {
    return buildUpdateMidprice(
      authority,
      this.addresses.midpriceOracle,
      params,
      this.programId
    );
  }
  /** Build update base spread instruction. */
  updateBaseSpread(authority, params) {
    return buildUpdateBaseSpread(
      authority,
      this.addresses.midpriceOracle,
      params,
      this.programId
    );
  }
  /** Build atomic update of midprice + base spread. */
  updateMidpriceAndBaseSpread(authority, params) {
    return buildUpdateMidpriceAndBaseSpread(
      authority,
      this.addresses.midpriceOracle,
      params,
      this.programId
    );
  }
  /** Build switch price curve instruction (hot path). */
  switchPriceCurve(authority, params) {
    return buildSwitchPriceCurve(
      authority,
      this.addresses.curveMeta,
      params,
      this.programId
    );
  }
  /** Build switch risk curve instruction (hot path). */
  switchRiskCurve(authority, params) {
    return buildSwitchRiskCurve(
      authority,
      this.addresses.curveMeta,
      params,
      this.programId
    );
  }
  /** Build submit curve updates instruction (hot path). */
  submitCurveUpdates(authority, sequence, ops) {
    return buildSubmitCurveUpdates(
      authority,
      this.addresses.curveUpdates,
      sequence,
      ops,
      this.programId
    );
  }
  /** Build apply curve updates instruction. */
  applyCurveUpdates(authority) {
    return buildApplyCurveUpdates(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      this.addresses.curveUpdates,
      this.programId
    );
  }
  /** Build nominate authority instruction. */
  nominateAuthority(authority, params) {
    return buildNominateAuthority(
      authority,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /** Build accept authority instruction. */
  acceptAuthority(newAuthority) {
    return buildAcceptAuthority(
      newAuthority,
      this.poolAddress,
      this.programId
    );
  }
  /** Build rotate quoting authority instruction (v2 disc 35). */
  rotateQuotingAuthority(authority, params) {
    return buildRotateQuotingAuthority(
      authority,
      this.poolAddress,
      this.addresses.midpriceOracle,
      this.addresses.curveMeta,
      this.addresses.curveUpdates,
      params,
      this.programId
    );
  }
  /** Build set pool state instruction. */
  setPoolState(authority, params) {
    return buildSetPoolState(
      authority,
      this.poolAddress,
      this.addresses.curveMeta,
      params,
      this.programId
    );
  }
  /** Build update delta staleness instruction. */
  updateDeltaStaleness(authority, params) {
    return buildUpdateDeltaStaleness(
      authority,
      this.poolAddress,
      this.addresses.curveMeta,
      params,
      this.programId
    );
  }
  /** Build initialize spread config instruction. */
  initializeSpreadConfig(payer, authority, params) {
    return buildInitializeSpreadConfig(
      payer,
      authority,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /** Build update spread config instruction (full replacement). */
  updateSpreadConfig(admin, params) {
    return buildUpdateSpreadConfig(
      admin,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /**
   * Fetch current spread triggers, append new ones, and return the update ix.
   * If a trigger for the same account already exists, its spreadBps is updated.
   */
  async addSpreadTriggers(admin, triggers) {
    const current = await this.fetchSpreadTriggers();
    const merged = [...current];
    for (const t of triggers) {
      const existing = merged.findIndex(
        (e) => e.account.equals(t.account)
      );
      if (existing >= 0) {
        merged[existing] = t;
      } else {
        merged.push(t);
      }
    }
    return this.updateSpreadConfig(admin, { triggers: merged });
  }
  /**
   * Fetch current spread triggers, remove the given accounts, and return the update ix.
   */
  async removeSpreadTriggers(admin, accounts) {
    const current = await this.fetchSpreadTriggers();
    const removeSet = new Set(accounts.map((a) => a.toBase58()));
    const filtered = current.filter(
      (t) => !removeSet.has(t.account.toBase58())
    );
    return this.updateSpreadConfig(admin, { triggers: filtered });
  }
  /** Fetch and decode the current spread config triggers from chain. */
  async fetchSpreadTriggers() {
    const [spreadConfigPda] = getSpreadConfigAddress(
      this.poolAddress,
      this.programId
    );
    const info = await this.connection.getAccountInfo(spreadConfigPda);
    if (!info) return [];
    const decoded = decodeSpreadConfig(info.data);
    return decoded.triggers;
  }
  /**
   * Build close pool instruction.
   *
   * `withdrawAtaX`/`withdrawAtaY` are the vault-sweep destinations and MUST be
   * the ATAs of `config.withdrawAuthority` (NOT the signing authority). They
   * are required whenever the corresponding vault has a non-zero balance.
   *
   * Token-2022 sides automatically get their mint appended so the vault sweep
   * uses TransferChecked (required for transfer-fee / hook / pausable mints).
   * For transfer-fee mints whose vault has accrued withheld fees, prefer
   * {@link closePoolInstructions}, which prepends the required
   * HarvestWithheldTokensToMint instruction(s).
   */
  closePool(authority, withdrawAtaX, withdrawAtaY) {
    return buildClosePool(
      authority,
      this.poolAddress,
      this.addresses.midpriceOracle,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      this.addresses.curveUpdates,
      this.addresses.vaultX,
      this.addresses.vaultY,
      this.config.tokenProgramX,
      this.config.tokenProgramY,
      this.config.spreadConfigInitialized,
      withdrawAtaX,
      withdrawAtaY,
      this.programId,
      this.needsCheckedTransferX ? this.config.mintX : void 0,
      this.needsCheckedTransferY ? this.config.mintY : void 0
    );
  }
  /**
   * Build the full close-pool instruction sequence.
   *
   * Token-2022 vaults of transfer-fee mints accrue withheld fees on incoming
   * transfers; CloseAccount rejects such vaults with
   * AccountHasWithheldTransferFees. This reads each Token-2022 vault and
   * prepends a permissionless HarvestWithheldTokensToMint only when the vault
   * actually holds withheld fees, then appends the ClosePool instruction.
   * Clean vaults (including any transfer-fee mint at 0 bps, e.g. USDG) get no
   * harvest — the instruction is a no-op there, and wallet transaction
   * scanners treat it as scam-token tooling.
   */
  async closePoolInstructions(authority, withdrawAtaX, withdrawAtaY) {
    const ixs = [];
    const legs = [
      {
        needsChecked: this.needsCheckedTransferX,
        mint: this.config.mintX,
        tokenProgram: this.config.tokenProgramX,
        vault: this.addresses.vaultX
      },
      {
        needsChecked: this.needsCheckedTransferY,
        mint: this.config.mintY,
        tokenProgram: this.config.tokenProgramY,
        vault: this.addresses.vaultY
      }
    ];
    for (const leg of legs) {
      if (!leg.needsChecked) continue;
      const info = await this.connection.getAccountInfo(leg.vault);
      if (!info) continue;
      const vault = _spltoken.unpackAccount.call(void 0, leg.vault, info, leg.tokenProgram);
      const withheld = _spltoken.getTransferFeeAmount.call(void 0, vault);
      if (withheld && withheld.withheldAmount > 0n) {
        ixs.push(
          _spltoken.createHarvestWithheldTokensToMintInstruction.call(void 0, 
            leg.mint,
            [leg.vault],
            leg.tokenProgram
          )
        );
      }
    }
    ixs.push(this.closePool(authority, withdrawAtaX, withdrawAtaY));
    return ixs;
  }
  // ==========================================================================
  // Governance (config admin) Instruction Builders
  // ==========================================================================
  /**
   * Build an UpdateDrawdownGuard instruction.
   *
   * Sets the pool's drawdown trip threshold and HWM rotation interval. Signer
   * must be `config.authority`. Reads the oracle and both vaults to anchor HWMs.
   */
  updateDrawdownGuard(authority, params) {
    return buildUpdateDrawdownGuard(
      authority,
      this.poolAddress,
      this.addresses.midpriceOracle,
      this.addresses.vaultX,
      this.addresses.vaultY,
      params,
      this.programId
    );
  }
  /**
   * Build a PokeDrawdownGuard instruction.
   *
   * Re-evaluates / trips the drawdown guard from current oracle + vault state
   * without changing config. Signer must be the **midprice oracle authority**
   * (not `config.authority`), and the guard must be enabled on a live pool.
   */
  pokeDrawdownGuard(authority) {
    return buildPokeDrawdownGuard(
      authority,
      this.poolAddress,
      this.addresses.midpriceOracle,
      this.addresses.vaultX,
      this.addresses.vaultY,
      this.programId
    );
  }
  /**
   * Build an UpdateFactorClamp instruction.
   *
   * Sets `config.combined_factor_max_q32` (must be >= 1.0 in Q32). Signer must
   * be `config.authority`.
   */
  updateFactorClamp(authority, params) {
    return buildUpdateFactorClamp(
      authority,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /**
   * Build a RotateDepositAuthority instruction.
   *
   * Signer may be the current `config.deposit_authority`, or `config.authority`
   * when `lp_self_rotate == false` (custodial mode).
   */
  rotateDepositAuthority(signer, params) {
    return buildRotateDepositAuthority(
      signer,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /**
   * Build a RotateWithdrawAuthority instruction.
   *
   * Signer may be the current `config.withdraw_authority`, or `config.authority`
   * when `lp_self_rotate == false`.
   */
  rotateWithdrawAuthority(signer, params) {
    return buildRotateWithdrawAuthority(
      signer,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /**
   * Build a RotateSpreadAdmin instruction.
   *
   * Sets `spread_config.admin`. Signer must be `config.authority` (the pool
   * admin, not the current spread admin).
   */
  rotateSpreadAdmin(authority, params) {
    return buildRotateSpreadAdmin(
      authority,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /**
   * Build a RotatePauseAuthority instruction.
   *
   * Sets `config.pause_authority`. Signer must be `config.authority`.
   */
  rotatePauseAuthority(authority, params) {
    return buildRotatePauseAuthority(
      authority,
      this.poolAddress,
      params,
      this.programId
    );
  }
  // ==========================================================================
  // Per-Pool Fee Config Instruction Builders
  // ==========================================================================
  /**
   * Build an InitializePoolFeeConfig instruction.
   *
   * Creates a pool-specific fee override (sets `config.has_pool_fee`). `feeAdmin`
   * is the GLOBAL fee admin and must sign alongside `payer`.
   */
  initializePoolFeeConfig(payer, feeAdmin, params) {
    return buildInitializePoolFeeConfig(
      payer,
      feeAdmin,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /**
   * Build an UpdatePoolFeeConfig instruction.
   *
   * Updates this pool's fee override. Pass `feePpm: null` / `feeRecipient: null`
   * to leave a field unchanged. Signer is the pool fee admin.
   */
  updatePoolFeeConfig(feeAdmin, params) {
    return buildUpdatePoolFeeConfig(
      feeAdmin,
      this.poolAddress,
      params,
      this.programId
    );
  }
  /**
   * Build a ClosePoolFeeConfig instruction.
   *
   * Reverts the pool from its per-pool fee back to the global fee, closes the
   * pool_fee_config PDA, and drains rent to `feeAdmin` (the GLOBAL fee admin).
   */
  closePoolFeeConfig(feeAdmin) {
    return buildClosePoolFeeConfig(
      feeAdmin,
      this.poolAddress,
      this.programId
    );
  }
  // ==========================================================================
  // Curve Update Validation
  // ==========================================================================
  /**
   * Build a ValidateCurveUpdates instruction (dry-run).
   *
   * Applies the queued curve ops in-place then intentionally reverts with
   * `ValidationSimulationSuccess`. Send via `simulateTransaction` to surface
   * per-op errors before paying CU for a real `applyCurveUpdates`. On-chain
   * state never changes.
   */
  validateCurveUpdates(authority) {
    return buildValidateCurveUpdates(
      authority,
      this.addresses.curveMeta,
      this.addresses.curvePrefabs,
      this.addresses.curveUpdates,
      this.programId
    );
  }
  // ==========================================================================
  // SOL Wrapping Convenience Methods
  // ==========================================================================
  /**
   * Build deposit instructions with automatic SOL wrapping.
   * If mint X or Y is native SOL, prepends wrap instructions for that side.
   */
  depositSol(user, params) {
    const ixs = [];
    if (isNativeMint(this.config.mintX) && params.amountX > 0n) {
      ixs.push(...wrapSolInstructions(user, params.amountX));
    }
    if (isNativeMint(this.config.mintY) && params.amountY > 0n) {
      ixs.push(...wrapSolInstructions(user, params.amountY));
    }
    ixs.push(this.deposit(user, params));
    return ixs;
  }
  /**
   * Build withdraw instructions with automatic SOL unwrapping.
   * Creates wSOL ATA (so program can transfer into it), withdraws, then closes ATA.
   */
  withdrawSol(user, params) {
    const ixs = [];
    const hasSol = isNativeMint(this.config.mintX) || isNativeMint(this.config.mintY);
    if (hasSol) {
      ixs.push(createWsolAtaInstruction(user));
    }
    ixs.push(this.withdraw(user, params));
    if (hasSol) {
      ixs.push(...unwrapSolInstructions(user));
    }
    return ixs;
  }
  /**
   * Build swap instructions with automatic SOL wrapping/unwrapping.
   * Wraps SOL if the input side is native, unwraps if the output side is native.
   */
  swapSol(user, params) {
    const ixs = [];
    const inputIsX = params.isX;
    const inputMint = inputIsX ? this.config.mintX : this.config.mintY;
    const outputMint = inputIsX ? this.config.mintY : this.config.mintX;
    if (isNativeMint(inputMint)) {
      ixs.push(...wrapSolInstructions(user, params.amountIn));
    }
    if (isNativeMint(outputMint)) {
      ixs.push(createWsolAtaInstruction(user));
    }
    ixs.push(this.swap(user, params));
    if (isNativeMint(outputMint)) {
      ixs.push(...unwrapSolInstructions(user));
    }
    return ixs;
  }
};
function evaluateRiskCurveAt(curve, pctQ32) {
  if (curve.points.length === 0) return 1;
  const n = curve.points.length;
  if (pctQ32 <= curve.points[0].amountIn) {
    return fromQ32(curve.points[0].priceFactorQ32);
  }
  if (pctQ32 >= curve.points[n - 1].amountIn) {
    return fromQ32(curve.points[n - 1].priceFactorQ32);
  }
  for (let i = 1; i < n; i++) {
    if (pctQ32 <= curve.points[i].amountIn) {
      const p0 = curve.points[i - 1];
      const p1 = curve.points[i];
      const y0 = fromQ32(p0.priceFactorQ32);
      const y1 = fromQ32(p1.priceFactorQ32);
      const x0 = Number(p0.amountIn);
      const x1 = Number(p1.amountIn);
      const x = Number(pctQ32);
      const t = (x - x0) / (x1 - x0);
      switch (p0.interpolation) {
        case 0 /* Step */:
          return y0;
        case 1 /* Linear */:
          return y0 + (y1 - y0) * t;
        case 2 /* MarginalStep */: {
          const c0 = x0 * y0;
          const marginal = (x1 * y1 - c0) / (x1 - x0);
          if (x === 0) return y0;
          return (c0 + marginal * (x - x0)) / x;
        }
        case 3 /* Hyperbolic */: {
          const k = readQ16_16(p0.params);
          if (k <= 0) return y0 + (y1 - y0) * t;
          const eased = t / (t + k * (1 - t));
          return y0 + (y1 - y0) * eased;
        }
        case 4 /* Quadratic */: {
          const k = readQ16_16Signed(p0.params);
          const eased = t + k * (t * t - t);
          return y0 + (y1 - y0) * eased;
        }
        case 5 /* Cubic */: {
          const a = readQ8_8Signed(p0.params, 0);
          const b = readQ8_8Signed(p0.params, 2);
          const eased = a * t * t * t + b * t * t + (1 - a - b) * t;
          return y0 + (y1 - y0) * eased;
        }
        default:
          return y0 + (y1 - y0) * t;
      }
    }
  }
  return fromQ32(curve.points[n - 1].priceFactorQ32);
}
function readQ16_16(params) {
  const raw = params[0] | params[1] << 8 | params[2] << 16 | params[3] << 24;
  return (raw >>> 0) / 65536;
}
function readQ16_16Signed(params) {
  const raw = params[0] | params[1] << 8 | params[2] << 16 | params[3] << 24;
  return (raw | 0) / 65536;
}
function readQ8_8Signed(params, offset) {
  const raw = params[offset] | params[offset + 1] << 8;
  const signed = raw > 32767 ? raw - 65536 : raw;
  return signed / 256;
}

// src/orderbook.ts
var MAX_ORDERS_PER_SIDE = MAX_SETCURVE_POINTS - 1;
function tokensToAtoms(tokens, decimals) {
  return BigInt(Math.round(tokens * 10 ** decimals));
}
function atomsToTokens(atoms, decimals) {
  return Number(atoms) / 10 ** decimals;
}
var HadronOrderbook = class _HadronOrderbook {
  constructor(pool, decimalsX, decimalsY, initialVaultX, initialVaultY) {
    this.bidOrders = [];
    this.askOrders = [];
    this.bidsDirty = false;
    this.asksDirty = false;
    this.priceCurvesSet = false;
    this.riskBidInitialized = false;
    this.riskAskInitialized = false;
    this.committedBidPoints = [];
    this.committedAskPoints = [];
    this.pool = pool;
    this.decimalsX = decimalsX;
    this.decimalsY = decimalsY;
    this.initialVaultX = initialVaultX;
    this.initialVaultY = initialVaultY;
  }
  // ==========================================================================
  // Static Factories
  // ==========================================================================
  /**
   * Load an orderbook from an existing on-chain pool.
   * Fetches decimals and vault balances, and reconstructs staged orders
   * from the active risk curves.
   */
  static async load(params) {
    const { connection, pool: poolAddress } = params;
    const hadron = await Hadron.load(connection, poolAddress);
    const [mintXInfo, mintYInfo, vaultXInfo, vaultYInfo] = await Promise.all([
      connection.getAccountInfo(hadron.config.mintX),
      connection.getAccountInfo(hadron.config.mintY),
      connection.getAccountInfo(hadron.addresses.vaultX),
      connection.getAccountInfo(hadron.addresses.vaultY)
    ]);
    if (!mintXInfo || !mintYInfo) throw new Error("Mint account(s) not found");
    if (!vaultXInfo || !vaultYInfo) throw new Error("Vault account(s) not found");
    const decimalsX = mintXInfo.data[44];
    const decimalsY = mintYInfo.data[44];
    const vaultXBalance = new DataView(
      vaultXInfo.data.buffer,
      vaultXInfo.data.byteOffset
    ).getBigUint64(64, true);
    const vaultYBalance = new DataView(
      vaultYInfo.data.buffer,
      vaultYInfo.data.byteOffset
    ).getBigUint64(64, true);
    const book = new _HadronOrderbook(
      hadron,
      decimalsX,
      decimalsY,
      vaultXBalance,
      vaultYBalance
    );
    book.reconstructFromCurves();
    return book;
  }
  /**
   * Wrap an existing Hadron instance as an orderbook.
   * Useful for tests or when you already have the pool loaded.
   */
  static fromPool(pool, decimalsX, decimalsY, initialVaultX, initialVaultY) {
    return new _HadronOrderbook(
      pool,
      decimalsX,
      decimalsY,
      initialVaultX,
      initialVaultY
    );
  }
  // ==========================================================================
  // Order Management
  // ==========================================================================
  /**
   * Place a new order. Orders are staged locally and pushed on-chain via `push()`.
   * Orders on each side are sorted by spreadBps ascending (tightest first).
   * @throws If more than 31 orders on a side.
   */
  placeOrder(params) {
    const orders = params.side === "bid" ? this.bidOrders : this.askOrders;
    if (orders.length >= MAX_ORDERS_PER_SIDE) {
      throw new Error(
        `Cannot place more than ${MAX_ORDERS_PER_SIDE} orders per side`
      );
    }
    orders.push({ size: params.size, spreadBps: params.spreadBps });
    orders.sort((a, b) => a.spreadBps - b.spreadBps);
    if (params.side === "bid") this.bidsDirty = true;
    else this.asksDirty = true;
  }
  /**
   * Amend an existing order at the given level index.
   * Re-sorts if spread changes.
   * @throws On invalid index.
   */
  amendOrder(params) {
    const orders = params.side === "bid" ? this.bidOrders : this.askOrders;
    if (params.level < 0 || params.level >= orders.length) {
      throw new Error(
        `Invalid level ${params.level} for ${params.side} (have ${orders.length} orders)`
      );
    }
    const order = orders[params.level];
    if (params.spreadBps !== void 0) order.spreadBps = params.spreadBps;
    if (params.size !== void 0) order.size = params.size;
    orders.sort((a, b) => a.spreadBps - b.spreadBps);
    if (params.side === "bid") this.bidsDirty = true;
    else this.asksDirty = true;
  }
  /**
   * Cancel the order at the given level index.
   * @throws On invalid index.
   */
  cancelOrder(params) {
    const orders = params.side === "bid" ? this.bidOrders : this.askOrders;
    if (params.level < 0 || params.level >= orders.length) {
      throw new Error(
        `Invalid level ${params.level} for ${params.side} (have ${orders.length} orders)`
      );
    }
    orders.splice(params.level, 1);
    if (params.side === "bid") this.bidsDirty = true;
    else this.asksDirty = true;
  }
  /** Cancel all orders on one or both sides. */
  cancelAll(side) {
    if (!side || side === "bid") {
      this.bidOrders = [];
      this.bidsDirty = true;
    }
    if (!side || side === "ask") {
      this.askOrders = [];
      this.asksDirty = true;
    }
  }
  /** Read-only access to staged bid orders. */
  getBids() {
    return this.bidOrders;
  }
  /** Read-only access to staged ask orders. */
  getAsks() {
    return this.askOrders;
  }
  // ==========================================================================
  // Push (orders → on-chain risk curves)
  // ==========================================================================
  /**
   * Build transaction instructions to push the current staged orders on-chain.
   *
   * 1. On first push, sets flat price curves (factor=1.0, Step interpolation).
   * 2. If risk curves not yet initialized, uses setRiskCurveAbsolute (sets headers).
   * 3. On subsequent pushes, computes a minimal diff and uses curve updates
   *    (submit + apply) when possible, falling back to full rewrite for large diffs.
   */
  push(authority) {
    const ixs = [];
    if (!this.priceCurvesSet) {
      const flatPoints = [
        { amountIn: 0n, priceFactor: 1, interpolation: 0 /* Step */ },
        {
          amountIn: BigInt("1000000000000000000"),
          priceFactor: 1,
          interpolation: 0 /* Step */
        }
      ];
      const [bidPriceIx, askPriceIx] = this.pool.setCurveBoth(authority, {
        bid: { defaultInterpolation: 0 /* Step */, points: flatPoints },
        ask: { defaultInterpolation: 0 /* Step */, points: flatPoints }
      });
      ixs.push(bidPriceIx, askPriceIx);
      this.priceCurvesSet = true;
    }
    if (!this.riskBidInitialized || !this.riskAskInitialized) {
      ixs.push(...this.buildFullRiskCurveIxs(authority));
    }
    const updateIxs = this.buildCurveUpdateIxs(authority);
    ixs.push(...updateIxs);
    this.bidsDirty = false;
    this.asksDirty = false;
    return ixs;
  }
  // ==========================================================================
  // Delegated Methods
  // ==========================================================================
  /** Build an updateMidprice instruction. */
  updateMidprice(authority, midprice, sequence) {
    const params = {
      midpriceQ32: BigInt(Math.floor(midprice * 2 ** 32)),
      sequence
    };
    return this.pool.updateMidprice(authority, params);
  }
  /** Build a deposit instruction. */
  deposit(user, params) {
    return this.pool.deposit(user, params);
  }
  /** Build a withdraw instruction. */
  withdraw(user, params) {
    return this.pool.withdraw(user, params);
  }
  /** Build a setPoolState instruction. */
  setPoolState(authority, params) {
    return this.pool.setPoolState(authority, params);
  }
  /**
   * Get the current book state by reading on-chain risk curves + midprice.
   * Reconstructs levels from curve points.
   */
  getBookState() {
    const midprice = this.pool.getMidprice();
    const curves = this.pool.getActiveCurves();
    const bids = this.curveSideToLevels(
      curves.riskBid,
      midprice,
      "bid",
      this.decimalsX
    );
    const asks = this.curveSideToLevels(
      curves.riskAsk,
      midprice,
      "ask",
      this.decimalsY
    );
    const base = atomsToTokens(this.initialVaultX, this.decimalsX);
    const quote = atomsToTokens(this.initialVaultY, this.decimalsY);
    return { midprice, bids, asks, inventory: { base, quote } };
  }
  // ==========================================================================
  // Curve Update Diff Algorithm
  // ==========================================================================
  /**
   * Compute the minimal set of curve update ops to transform `committed` into `desired`.
   *
   * Algorithm:
   * 1. Scan left-to-right for factor-only diffs (amountIn unchanged) → Edit ops.
   * 2. At the first structural diff (amountIn changed or length mismatch),
   *    truncate-and-rebuild: remove all from that index to end, then add desired.
   * 3. Returns the ops array.
   */
  static computeCurveOps(committed, desired, curveType) {
    const ops = [];
    const minLen = Math.min(committed.length, desired.length);
    let structuralDiffIndex = minLen;
    for (let i = 0; i < minLen; i++) {
      const c = committed[i];
      const d = desired[i];
      if (c.amountIn !== d.amountIn) {
        structuralDiffIndex = i;
        break;
      }
      if (c.priceFactorQ32 !== d.priceFactorQ32 || c.interpolation !== d.interpolation) {
        ops.push({
          curveType,
          opKind: 0 /* Edit */,
          pointIndex: i,
          interpolation: d.interpolation,
          amountIn: d.amountIn,
          priceFactorQ32: d.priceFactorQ32,
          params: d.params,
          targetSlot: 0,
          expectedXIn: c.amountIn
        });
      }
    }
    if (structuralDiffIndex === minLen && committed.length !== desired.length) {
      structuralDiffIndex = minLen;
    }
    if (structuralDiffIndex === minLen && committed.length === desired.length) {
      return ops;
    }
    const numToRemove = committed.length - structuralDiffIndex;
    for (let i = 0; i < numToRemove; i++) {
      ops.push({
        curveType,
        opKind: 2 /* Remove */,
        pointIndex: structuralDiffIndex,
        interpolation: 0 /* Step */,
        amountIn: 0n,
        priceFactorQ32: 0n,
        params: new Uint8Array(4),
        targetSlot: 0,
        expectedXIn: _nullishCoalesce(_optionalChain([committed, 'access', _11 => _11[structuralDiffIndex + i], 'optionalAccess', _12 => _12.amountIn]), () => ( 0n))
      });
    }
    for (let i = structuralDiffIndex; i < desired.length; i++) {
      const d = desired[i];
      ops.push({
        curveType,
        opKind: 1 /* Add */,
        pointIndex: i,
        interpolation: d.interpolation,
        amountIn: d.amountIn,
        priceFactorQ32: d.priceFactorQ32,
        params: d.params,
        targetSlot: 0,
        expectedXIn: 0n
      });
    }
    return ops;
  }
  // ==========================================================================
  // Internal Helpers
  // ==========================================================================
  /** Convert SetRiskCurveAbsolutePointInput[] to CurvePoint[] for diffing. */
  static toCommittedPoints(points) {
    return points.map((p) => ({
      amountIn: p.vaultBalance,
      priceFactorQ32: toQ32(p.priceFactor),
      interpolation: _nullishCoalesce(p.interpolation, () => ( 0)) /* Step */,
      params: _nullishCoalesce(p.params, () => ( new Uint8Array(4)))
    }));
  }
  /**
   * Build full setRiskCurveAbsolute instructions for sides that need initialization.
   * After sending, snapshots committed points and marks sides as initialized.
   */
  buildFullRiskCurveIxs(authority) {
    const ixs = [];
    const hasBids = this.bidOrders.length > 0;
    const hasAsks = this.askOrders.length > 0;
    const bidPoints = hasBids ? this.buildBidRiskPoints() : void 0;
    const askPoints = hasAsks ? this.buildAskRiskPoints() : void 0;
    if (bidPoints && askPoints && !this.riskBidInitialized && !this.riskAskInitialized) {
      const [bidIx, askIx] = this.pool.setRiskCurveAbsoluteBoth(authority, {
        bid: {
          defaultInterpolation: 0 /* Step */,
          points: bidPoints,
          riskMode: 1 /* Integrated */
        },
        ask: {
          defaultInterpolation: 0 /* Step */,
          points: askPoints,
          riskMode: 1 /* Integrated */
        }
      });
      ixs.push(bidIx, askIx);
      this.committedBidPoints = _HadronOrderbook.toCommittedPoints(bidPoints);
      this.committedAskPoints = _HadronOrderbook.toCommittedPoints(askPoints);
      this.riskBidInitialized = true;
      this.riskAskInitialized = true;
    } else {
      if (bidPoints && !this.riskBidInitialized) {
        ixs.push(
          this.pool.setRiskCurveAbsolute(authority, {
            side: 0 /* Bid */,
            defaultInterpolation: 0 /* Step */,
            points: bidPoints,
            riskMode: 1 /* Integrated */
          })
        );
        this.committedBidPoints = _HadronOrderbook.toCommittedPoints(bidPoints);
        this.riskBidInitialized = true;
      }
      if (askPoints && !this.riskAskInitialized) {
        ixs.push(
          this.pool.setRiskCurveAbsolute(authority, {
            side: 1 /* Ask */,
            defaultInterpolation: 0 /* Step */,
            points: askPoints,
            riskMode: 1 /* Integrated */
          })
        );
        this.committedAskPoints = _HadronOrderbook.toCommittedPoints(askPoints);
        this.riskAskInitialized = true;
      }
    }
    return ixs;
  }
  /**
   * Build curve update instructions (submit + apply pairs) for both sides.
   * Falls back to full setRiskCurve rewrite if diff is too large.
   */
  buildCurveUpdateIxs(authority) {
    const bidPoints = this.buildBidRiskPoints();
    const askPoints = this.buildAskRiskPoints();
    const desiredBid = _HadronOrderbook.toCommittedPoints(bidPoints);
    const desiredAsk = _HadronOrderbook.toCommittedPoints(askPoints);
    const bidOps = this.riskBidInitialized ? _HadronOrderbook.computeCurveOps(
      this.committedBidPoints,
      desiredBid,
      2 /* RiskBid */
    ) : [];
    const askOps = this.riskAskInitialized ? _HadronOrderbook.computeCurveOps(
      this.committedAskPoints,
      desiredAsk,
      3 /* RiskAsk */
    ) : [];
    const allOps = [...bidOps, ...askOps];
    if (allOps.length === 0) {
      return [];
    }
    if (allOps.length > MAX_CURVE_UPDATE_OPS * 4) {
      return this.buildFallbackRewrite(authority, bidPoints, askPoints);
    }
    const ixs = [];
    for (let i = 0; i < allOps.length; i += MAX_CURVE_UPDATE_OPS) {
      const batch = allOps.slice(i, i + MAX_CURVE_UPDATE_OPS);
      ixs.push(this.pool.submitCurveUpdates(authority, 0n, batch));
      ixs.push(this.pool.applyCurveUpdates(authority));
    }
    this.committedBidPoints = desiredBid;
    this.committedAskPoints = desiredAsk;
    return ixs;
  }
  /** Fallback: full setRiskCurve rewrite for both sides. */
  buildFallbackRewrite(authority, bidPoints, askPoints) {
    const [bidIx, askIx] = this.pool.setRiskCurveAbsoluteBoth(authority, {
      bid: {
        defaultInterpolation: 0 /* Step */,
        points: bidPoints,
        riskMode: 1 /* Integrated */
      },
      ask: {
        defaultInterpolation: 0 /* Step */,
        points: askPoints,
        riskMode: 1 /* Integrated */
      }
    });
    this.committedBidPoints = _HadronOrderbook.toCommittedPoints(bidPoints);
    this.committedAskPoints = _HadronOrderbook.toCommittedPoints(askPoints);
    return [bidIx, askIx];
  }
  buildBidRiskPoints() {
    const points = [];
    let cumulativeAtoms = this.initialVaultX;
    for (const order of this.bidOrders) {
      const priceFactor = 1 - order.spreadBps / 1e4;
      points.push({
        vaultBalance: cumulativeAtoms,
        priceFactor,
        interpolation: 0 /* Step */
      });
      cumulativeAtoms += tokensToAtoms(order.size, this.decimalsX);
    }
    const lastFactor = this.bidOrders.length > 0 ? 1 - this.bidOrders[this.bidOrders.length - 1].spreadBps / 1e4 : 1;
    points.push({
      vaultBalance: cumulativeAtoms,
      priceFactor: lastFactor,
      interpolation: 0 /* Step */
    });
    return points;
  }
  buildAskRiskPoints() {
    const points = [];
    let cumulativeAtoms = this.initialVaultY;
    const midprice = this.pool.getMidprice();
    for (const order of this.askOrders) {
      const priceFactor = 1 - order.spreadBps / 1e4;
      points.push({
        vaultBalance: cumulativeAtoms,
        priceFactor,
        interpolation: 0 /* Step */
      });
      const quoteSizeTokens = order.size * midprice;
      cumulativeAtoms += tokensToAtoms(quoteSizeTokens, this.decimalsY);
    }
    const lastFactor = this.askOrders.length > 0 ? 1 - this.askOrders[this.askOrders.length - 1].spreadBps / 1e4 : 1;
    points.push({
      vaultBalance: cumulativeAtoms,
      priceFactor: lastFactor,
      interpolation: 0 /* Step */
    });
    return points;
  }
  /**
   * Reconstruct staged orders from on-chain risk curves (after load).
   * Assumes curves were set via this class (CurveXMode.Alternate, RiskMode.Integrated, Step).
   */
  reconstructFromCurves() {
    const curves = this.pool.getActiveCurves();
    if (curves.riskBid.xMode === 1 /* Alternate */ && curves.riskBid.riskMode === 1 /* Integrated */ && curves.riskBid.numPoints >= 2) {
      this.riskBidInitialized = true;
      this.priceCurvesSet = true;
      this.committedBidPoints = curves.riskBid.points.map((p) => ({
        amountIn: p.amountIn,
        priceFactorQ32: p.priceFactorQ32,
        interpolation: p.interpolation,
        params: new Uint8Array(p.params)
      }));
    }
    if (curves.riskAsk.xMode === 1 /* Alternate */ && curves.riskAsk.riskMode === 1 /* Integrated */ && curves.riskAsk.numPoints >= 2) {
      this.riskAskInitialized = true;
      this.priceCurvesSet = true;
      this.committedAskPoints = curves.riskAsk.points.map((p) => ({
        amountIn: p.amountIn,
        priceFactorQ32: p.priceFactorQ32,
        interpolation: p.interpolation,
        params: new Uint8Array(p.params)
      }));
    }
    this.bidOrders = this.reconstructSide(
      curves.riskBid,
      "bid",
      this.initialVaultX,
      this.decimalsX
    );
    this.askOrders = this.reconstructSide(
      curves.riskAsk,
      "ask",
      this.initialVaultY,
      this.decimalsY
    );
  }
  reconstructSide(curve, side, initialVault, decimals) {
    if (curve.xMode !== 1 /* Alternate */ || curve.riskMode !== 1 /* Integrated */) {
      return [];
    }
    const orders = [];
    const points = curve.points;
    for (let i = 0; i < points.length - 1; i++) {
      const pt = points[i];
      const nextPt = points[i + 1];
      const priceFactor = fromQ32(pt.priceFactorQ32);
      const spreadBps = Math.round((1 - priceFactor) * 1e4);
      const sizeAtoms = nextPt.amountIn - pt.amountIn;
      let sizeTokens;
      if (side === "ask") {
        const midprice = this.pool.getMidprice();
        sizeTokens = atomsToTokens(sizeAtoms, decimals) / midprice;
      } else {
        sizeTokens = atomsToTokens(sizeAtoms, decimals);
      }
      orders.push({ size: sizeTokens, spreadBps });
    }
    return orders;
  }
  curveSideToLevels(curve, midprice, side, decimals) {
    if (curve.numPoints < 2) return [];
    const levels = [];
    let cumulative = 0;
    const points = curve.points;
    for (let i = 0; i < points.length - 1; i++) {
      const pt = points[i];
      const nextPt = points[i + 1];
      const priceFactor = fromQ32(pt.priceFactorQ32);
      const price = midprice * priceFactor;
      const sizeAtoms = nextPt.amountIn - pt.amountIn;
      let sizeTokens;
      if (side === "ask") {
        sizeTokens = atomsToTokens(sizeAtoms, decimals) / midprice;
      } else {
        sizeTokens = atomsToTokens(sizeAtoms, decimals);
      }
      cumulative += sizeTokens;
      levels.push({ price, size: sizeTokens, cumulative });
    }
    return levels;
  }
};

// src/errors.ts
var HadronError = /* @__PURE__ */ ((HadronError2) => {
  HadronError2[HadronError2["NotSigner"] = 0] = "NotSigner";
  HadronError2[HadronError2["InvalidOwner"] = 1] = "InvalidOwner";
  HadronError2[HadronError2["InvalidAccountData"] = 2] = "InvalidAccountData";
  HadronError2[HadronError2["InvalidAddress"] = 3] = "InvalidAddress";
  HadronError2[HadronError2["Unauthorized"] = 4] = "Unauthorized";
  HadronError2[HadronError2["PoolNotInitialized"] = 5] = "PoolNotInitialized";
  HadronError2[HadronError2["PoolAlreadyInitialized"] = 6] = "PoolAlreadyInitialized";
  HadronError2[HadronError2["InsufficientLiquidity"] = 7] = "InsufficientLiquidity";
  HadronError2[HadronError2["SlippageExceeded"] = 8] = "SlippageExceeded";
  HadronError2[HadronError2["CurvePointsNotSorted"] = 9] = "CurvePointsNotSorted";
  HadronError2[HadronError2["CurveDuplicateX"] = 10] = "CurveDuplicateX";
  HadronError2[HadronError2["CurveTooManyPoints"] = 11] = "CurveTooManyPoints";
  HadronError2[HadronError2["CurveEmpty"] = 12] = "CurveEmpty";
  HadronError2[HadronError2["InvalidMidprice"] = 13] = "InvalidMidprice";
  HadronError2[HadronError2["MathOverflow"] = 14] = "MathOverflow";
  HadronError2[HadronError2["ZeroSwapAmount"] = 15] = "ZeroSwapAmount";
  HadronError2[HadronError2["PoolPaused"] = 16] = "PoolPaused";
  HadronError2[HadronError2["FeeConfigAlreadyInitialized"] = 17] = "FeeConfigAlreadyInitialized";
  HadronError2[HadronError2["FeeConfigNotInitialized"] = 18] = "FeeConfigNotInitialized";
  HadronError2[HadronError2["NotFeeAdmin"] = 19] = "NotFeeAdmin";
  HadronError2[HadronError2["InvalidFeePpm"] = 20] = "InvalidFeePpm";
  HadronError2[HadronError2["RiskCurveXOutOfRange"] = 21] = "RiskCurveXOutOfRange";
  HadronError2[HadronError2["SequenceTooOld"] = 22] = "SequenceTooOld";
  HadronError2[HadronError2["PrefabSlotOutOfRange"] = 23] = "PrefabSlotOutOfRange";
  HadronError2[HadronError2["PrefabSlotNotInitialized"] = 25] = "PrefabSlotNotInitialized";
  HadronError2[HadronError2["SpreadConfigAlreadyInitialized"] = 27] = "SpreadConfigAlreadyInitialized";
  HadronError2[HadronError2["SpreadConfigNotInitialized"] = 28] = "SpreadConfigNotInitialized";
  HadronError2[HadronError2["TooManySpreadTriggers"] = 29] = "TooManySpreadTriggers";
  HadronError2[HadronError2["SpreadConfigRequired"] = 30] = "SpreadConfigRequired";
  HadronError2[HadronError2["SpreadConfigInvalidOwner"] = 31] = "SpreadConfigInvalidOwner";
  HadronError2[HadronError2["ConfigInvalidOwner"] = 32] = "ConfigInvalidOwner";
  HadronError2[HadronError2["MidpriceOracleInvalidOwner"] = 33] = "MidpriceOracleInvalidOwner";
  HadronError2[HadronError2["CurveMetaInvalidOwner"] = 34] = "CurveMetaInvalidOwner";
  HadronError2[HadronError2["CurvePrefabsInvalidOwner"] = 35] = "CurvePrefabsInvalidOwner";
  HadronError2[HadronError2["FeeConfigInvalidOwner"] = 36] = "FeeConfigInvalidOwner";
  HadronError2[HadronError2["StaleOracle"] = 37] = "StaleOracle";
  HadronError2[HadronError2["Expired"] = 38] = "Expired";
  HadronError2[HadronError2["NoPendingNomination"] = 39] = "NoPendingNomination";
  HadronError2[HadronError2["NominationExpired"] = 40] = "NominationExpired";
  HadronError2[HadronError2["InvalidNominee"] = 41] = "InvalidNominee";
  HadronError2[HadronError2["NominateSelf"] = 42] = "NominateSelf";
  HadronError2[HadronError2["InvalidNominationExpiry"] = 43] = "InvalidNominationExpiry";
  HadronError2[HadronError2["InvalidTokenProgram"] = 44] = "InvalidTokenProgram";
  HadronError2[HadronError2["PriceCurveXOutOfRange"] = 45] = "PriceCurveXOutOfRange";
  HadronError2[HadronError2["CurveUpdatesBufferFull"] = 46] = "CurveUpdatesBufferFull";
  HadronError2[HadronError2["CurveUpdatesInvalidOwner"] = 47] = "CurveUpdatesInvalidOwner";
  HadronError2[HadronError2["CurveMetaMismatch"] = 48] = "CurveMetaMismatch";
  HadronError2[HadronError2["InvalidRiskMode"] = 50] = "InvalidRiskMode";
  HadronError2[HadronError2["IntegratedRiskUnsupportedInterpolation"] = 51] = "IntegratedRiskUnsupportedInterpolation";
  HadronError2[HadronError2["VaultsNotEmpty"] = 52] = "VaultsNotEmpty";
  HadronError2[HadronError2["InvalidPoolState"] = 53] = "InvalidPoolState";
  HadronError2[HadronError2["MaxPrefabSlotsOutOfRange"] = 54] = "MaxPrefabSlotsOutOfRange";
  HadronError2[HadronError2["MaxCurvePointsOutOfRange"] = 55] = "MaxCurvePointsOutOfRange";
  HadronError2[HadronError2["VaultAddressMismatch"] = 56] = "VaultAddressMismatch";
  HadronError2[HadronError2["VaultTokenAccountTooSmall"] = 57] = "VaultTokenAccountTooSmall";
  HadronError2[HadronError2["UserTokenAccountTooSmall"] = 58] = "UserTokenAccountTooSmall";
  HadronError2[HadronError2["VaultMintMismatch"] = 59] = "VaultMintMismatch";
  HadronError2[HadronError2["UserTokenAccountInvalid"] = 60] = "UserTokenAccountInvalid";
  HadronError2[HadronError2["ConcavityViolation"] = 61] = "ConcavityViolation";
  HadronError2[HadronError2["PriceFactorTooLarge"] = 62] = "PriceFactorTooLarge";
  HadronError2[HadronError2["PoolFeeConfigAlreadyInitialized"] = 63] = "PoolFeeConfigAlreadyInitialized";
  HadronError2[HadronError2["PoolFeeConfigNotInitialized"] = 64] = "PoolFeeConfigNotInitialized";
  HadronError2[HadronError2["AssetFeeCapExceeded"] = 66] = "AssetFeeCapExceeded";
  HadronError2[HadronError2["CurveUpdateSlotMismatch"] = 67] = "CurveUpdateSlotMismatch";
  HadronError2[HadronError2["RiskCurveMonotonicityViolation"] = 68] = "RiskCurveMonotonicityViolation";
  HadronError2[HadronError2["InvalidTriggerSpreadBps"] = 70] = "InvalidTriggerSpreadBps";
  HadronError2[HadronError2["PoolAlreadyClosed"] = 71] = "PoolAlreadyClosed";
  HadronError2[HadronError2["CurveUpdateExpectedXMismatch"] = 72] = "CurveUpdateExpectedXMismatch";
  HadronError2[HadronError2["ValidationSimulationSuccess"] = 73] = "ValidationSimulationSuccess";
  HadronError2[HadronError2["NotDepositAuthority"] = 74] = "NotDepositAuthority";
  HadronError2[HadronError2["NotWithdrawAuthority"] = 75] = "NotWithdrawAuthority";
  HadronError2[HadronError2["RotateToZero"] = 76] = "RotateToZero";
  HadronError2[HadronError2["RotateNoOp"] = 77] = "RotateNoOp";
  HadronError2[HadronError2["DrawdownGuardDisabled"] = 78] = "DrawdownGuardDisabled";
  return HadronError2;
})(HadronError || {});
var HADRON_ERROR_MESSAGES = {
  [0 /* NotSigner */]: "Not the signer",
  [1 /* InvalidOwner */]: "Invalid account owner",
  [2 /* InvalidAccountData */]: "Invalid account data",
  [3 /* InvalidAddress */]: "Invalid address",
  [4 /* Unauthorized */]: "Unauthorized: not the pool authority",
  [5 /* PoolNotInitialized */]: "Pool not initialized",
  [6 /* PoolAlreadyInitialized */]: "Pool already initialized",
  [7 /* InsufficientLiquidity */]: "Insufficient liquidity",
  [8 /* SlippageExceeded */]: "Slippage exceeded",
  [9 /* CurvePointsNotSorted */]: "Invalid curve: points not sorted",
  [10 /* CurveDuplicateX */]: "Invalid curve: duplicate x values",
  [11 /* CurveTooManyPoints */]: "Invalid curve: too many points",
  [12 /* CurveEmpty */]: "Invalid curve: empty curve",
  [13 /* InvalidMidprice */]: "Invalid midprice",
  [14 /* MathOverflow */]: "Overflow in calculation",
  [15 /* ZeroSwapAmount */]: "Swap amount is zero",
  [16 /* PoolPaused */]: "Pool is paused",
  [17 /* FeeConfigAlreadyInitialized */]: "Fee config already initialized",
  [18 /* FeeConfigNotInitialized */]: "Fee config not initialized",
  [19 /* NotFeeAdmin */]: "Not the fee admin",
  [20 /* InvalidFeePpm */]: "Invalid fee PPM",
  [21 /* RiskCurveXOutOfRange */]: "Risk curve x-value out of range (must be 0 to Q32_ONE)",
  [22 /* SequenceTooOld */]: "Sequence too old - must be >= current sequence",
  [23 /* PrefabSlotOutOfRange */]: "Prefab slot index out of range (must be 0 to max_prefab_slots-1)",
  [25 /* PrefabSlotNotInitialized */]: "Prefab slot not initialized",
  [27 /* SpreadConfigAlreadyInitialized */]: "Spread config already initialized",
  [28 /* SpreadConfigNotInitialized */]: "Spread config not initialized",
  [29 /* TooManySpreadTriggers */]: "Too many spread triggers",
  [30 /* SpreadConfigRequired */]: "Spread config required - pool has spread config initialized but it was not provided",
  [31 /* SpreadConfigInvalidOwner */]: "Spread config account not owned by program",
  [32 /* ConfigInvalidOwner */]: "Config account not owned by program",
  [33 /* MidpriceOracleInvalidOwner */]: "MidpriceOracle account not owned by program",
  [34 /* CurveMetaInvalidOwner */]: "CurveMeta account not owned by program",
  [35 /* CurvePrefabsInvalidOwner */]: "CurvePrefabs account not owned by program",
  [36 /* FeeConfigInvalidOwner */]: "FeeConfig account not owned by program",
  [37 /* StaleOracle */]: "Stale oracle",
  [38 /* Expired */]: "Transaction expired",
  [39 /* NoPendingNomination */]: "No pending authority nomination",
  [40 /* NominationExpired */]: "Authority nomination has expired",
  [41 /* InvalidNominee */]: "Signer is not the nominated authority",
  [42 /* NominateSelf */]: "Cannot nominate self as new authority",
  [43 /* InvalidNominationExpiry */]: "Nomination expiry must be in the future",
  [44 /* InvalidTokenProgram */]: "Invalid token program (must be SPL Token or Token2022)",
  [45 /* PriceCurveXOutOfRange */]: "Price curve x-value out of range (must be 0 to Q32_ONE in percent mode)",
  [46 /* CurveUpdatesBufferFull */]: "Curve updates buffer is full",
  [47 /* CurveUpdatesInvalidOwner */]: "CurveUpdates account not owned by program",
  [48 /* CurveMetaMismatch */]: "CurveMeta address mismatch: account does not belong to this pool",
  [50 /* InvalidRiskMode */]: "Invalid risk mode (must be 0=Virtual or 1=Integrated)",
  [51 /* IntegratedRiskUnsupportedInterpolation */]: "Integrated risk mode is not supported for this interpolation type",
  [52 /* VaultsNotEmpty */]: "Vaults must be empty before closing pool",
  [53 /* InvalidPoolState */]: "Invalid pool state (must be 1=Initialized, 2=Paused, or 3=RejectSwaps, and different from current)",
  [54 /* MaxPrefabSlotsOutOfRange */]: "Max prefab slots out of range (must be 1-16)",
  [55 /* MaxCurvePointsOutOfRange */]: "Max curve points out of range (must be 1-128)",
  [56 /* VaultAddressMismatch */]: "Vault address does not match derived PDA",
  [57 /* VaultTokenAccountTooSmall */]: "Vault token account data too small",
  [58 /* UserTokenAccountTooSmall */]: "User token account data too small",
  [59 /* VaultMintMismatch */]: "Vault token account mint does not match config",
  [60 /* UserTokenAccountInvalid */]: "User token account has wrong owner or mint",
  [61 /* ConcavityViolation */]: "Curve interpolation params violate output concavity constraints",
  [62 /* PriceFactorTooLarge */]: "Price factor exceeds 1.0 (Q32_ONE) \u2014 price curves must have factors <= 1.0",
  [63 /* PoolFeeConfigAlreadyInitialized */]: "Pool fee config already initialized",
  [64 /* PoolFeeConfigNotInitialized */]: "Pool fee config not initialized",
  [66 /* AssetFeeCapExceeded */]: "Asset fee cap exceeded",
  [67 /* CurveUpdateSlotMismatch */]: "Curve update target_slot does not match active slot for the op's curve_type",
  [68 /* RiskCurveMonotonicityViolation */]: "Risk curve is not monotone (segment shape or factor sequence)",
  [70 /* InvalidTriggerSpreadBps */]: "Spread trigger spread_bps must be non-zero",
  [71 /* PoolAlreadyClosed */]: "Pool is closed (terminal state); operation not permitted",
  [72 /* CurveUpdateExpectedXMismatch */]: "Curve update expected_x_in does not match the current point at point_index",
  [73 /* ValidationSimulationSuccess */]: "ValidateCurveUpdates dry-run succeeded",
  [74 /* NotDepositAuthority */]: "Not the deposit authority",
  [75 /* NotWithdrawAuthority */]: "Not the withdraw authority",
  [76 /* RotateToZero */]: "Rotation target is the zero pubkey",
  [77 /* RotateNoOp */]: "Rotation target equals the current authority (no-op)",
  [78 /* DrawdownGuardDisabled */]: "Drawdown guard is disabled"
};
function getHadronErrorMessage(code) {
  return HADRON_ERROR_MESSAGES[code];
}

// src/helpers/concavity.ts
var ConcavityViolationError = class extends Error {
  constructor(message) {
    super(`Concavity violation: ${message}`);
    this.name = "ConcavityViolationError";
  }
};
function validateCurveConcavity(points) {
  const chain = points.map((p) => ({
    x: p.x,
    f: p.factorQ32,
    interp: p.interpolation,
    params: p.params
  }));
  const issues = validatePriceCurve(chain, "native");
  if (issues.length > 0) throw new ConcavityViolationError(formatIssue(issues[0]));
}

// src/helpers/token.ts





async function getOrCreateAta(connection, mint, owner, payer, tokenProgram = _spltoken.TOKEN_PROGRAM_ID, allowOwnerOffCurve = false) {
  const address = _spltoken.getAssociatedTokenAddressSync.call(void 0, 
    mint,
    owner,
    allowOwnerOffCurve,
    tokenProgram
  );
  const account = await connection.getAccountInfo(address);
  if (account) {
    return { address, instruction: null };
  }
  const instruction = _spltoken.createAssociatedTokenAccountInstruction.call(void 0, 
    payer,
    address,
    owner,
    mint,
    tokenProgram
  );
  return { address, instruction };
}







































































































































exports.ABSOLUTE_MAX_CURVE_POINTS = ABSOLUTE_MAX_CURVE_POINTS; exports.ABSOLUTE_MAX_PREFAB_SLOTS = ABSOLUTE_MAX_PREFAB_SLOTS; exports.ASSET_FEE_ENTRY_SIZE = ASSET_FEE_ENTRY_SIZE; exports.ASSET_FEE_PAIR_COUNT_SIZE = ASSET_FEE_PAIR_COUNT_SIZE; exports.CONFIG_SEED = CONFIG_SEED; exports.CONFIG_SIZE = CONFIG_SIZE; exports.CURVE_META_SEED = CURVE_META_SEED; exports.CURVE_META_SIZE = CURVE_META_SIZE; exports.CURVE_POINT_LEN = CURVE_POINT_LEN; exports.CURVE_PREFABS_SEED = CURVE_PREFABS_SEED; exports.CURVE_SIDE_HEADER = CURVE_SIDE_HEADER; exports.CURVE_UPDATES_SEED = CURVE_UPDATES_SEED; exports.CURVE_UPDATES_SIZE = CURVE_UPDATES_SIZE; exports.CURVE_UPDATE_OP_DATA_SIZE = CURVE_UPDATE_OP_DATA_SIZE; exports.CURVE_UPDATE_OP_SIZE = CURVE_UPDATE_OP_SIZE; exports.ConcavityViolationError = ConcavityViolationError; exports.CurveRejectedError = CurveRejectedError; exports.CurveType = CurveType; exports.CurveUpdateOpKind = CurveUpdateOpKind; exports.CurveXMode = CurveXMode; exports.DEFAULT_MAX_CURVE_POINTS = DEFAULT_MAX_CURVE_POINTS; exports.DEFAULT_MAX_PREFAB_SLOTS = DEFAULT_MAX_PREFAB_SLOTS; exports.DecoderError = DecoderError; exports.Discriminator = Discriminator; exports.FEE_CONFIG_SEED = FEE_CONFIG_SEED; exports.FEE_CONFIG_SIZE = FEE_CONFIG_SIZE; exports.HADRON_ERROR_MESSAGES = HADRON_ERROR_MESSAGES; exports.HADRON_PROGRAM_ID = HADRON_PROGRAM_ID; exports.Hadron = Hadron; exports.HadronError = HadronError; exports.HadronOrderbook = HadronOrderbook; exports.Interpolation = Interpolation; exports.MAX_ASSET_FEES = MAX_ASSET_FEES; exports.MAX_CURVE_UPDATE_OPS = MAX_CURVE_UPDATE_OPS; exports.MAX_SETCURVE_POINTS = MAX_SETCURVE_POINTS; exports.MAX_SPREAD_ACCOUNTS = MAX_SPREAD_ACCOUNTS; exports.MIDPRICE_ORACLE_SEED = MIDPRICE_ORACLE_SEED; exports.MIDPRICE_ORACLE_SIZE = MIDPRICE_ORACLE_SIZE; exports.NATIVE_MINT = _spltoken.NATIVE_MINT; exports.OracleMode = OracleMode; exports.POINT_DATA_SIZE = POINT_DATA_SIZE; exports.PoolState = PoolState; exports.Q32_ONE = Q32_ONE; exports.RiskMode = RiskMode; exports.SPREAD_CONFIG_SEED = SPREAD_CONFIG_SEED; exports.SPREAD_TRIGGER_IX_DATA_SIZE = SPREAD_TRIGGER_IX_DATA_SIZE; exports.Side = Side; exports.applyProtocolFee = applyProtocolFee; exports.assertCurveAccepted = assertCurveAccepted; exports.buildAcceptAuthority = buildAcceptAuthority; exports.buildAllocateCurvePrefabs = buildAllocateCurvePrefabs; exports.buildApplyCurveUpdates = buildApplyCurveUpdates; exports.buildClosePool = buildClosePool; exports.buildClosePoolFeeConfig = buildClosePoolFeeConfig; exports.buildDeposit = buildDeposit; exports.buildFeeRecipientAtaIxs = buildFeeRecipientAtaIxs; exports.buildInitialize = buildInitialize; exports.buildInitializeFeeConfig = buildInitializeFeeConfig; exports.buildInitializePoolFeeConfig = buildInitializePoolFeeConfig; exports.buildInitializeSpreadConfig = buildInitializeSpreadConfig; exports.buildNominateAuthority = buildNominateAuthority; exports.buildPokeDrawdownGuard = buildPokeDrawdownGuard; exports.buildRemoveSpreadTriggers = buildRemoveSpreadTriggers; exports.buildRotateDepositAuthority = buildRotateDepositAuthority; exports.buildRotateFeeAdmin = buildRotateFeeAdmin; exports.buildRotatePauseAuthority = buildRotatePauseAuthority; exports.buildRotateQuotingAuthority = buildRotateQuotingAuthority; exports.buildRotateSpreadAdmin = buildRotateSpreadAdmin; exports.buildRotateWithdrawAuthority = buildRotateWithdrawAuthority; exports.buildSetAssetFee = buildSetAssetFee; exports.buildSetCurve = buildSetCurve; exports.buildSetCurveBoth = buildSetCurveBoth; exports.buildSetPoolState = buildSetPoolState; exports.buildSetRiskCurve = buildSetRiskCurve; exports.buildSetRiskCurveAbsolute = buildSetRiskCurveAbsolute; exports.buildSetRiskCurveAbsoluteBoth = buildSetRiskCurveAbsoluteBoth; exports.buildSetRiskCurveBoth = buildSetRiskCurveBoth; exports.buildSubmitCurveUpdates = buildSubmitCurveUpdates; exports.buildSwapExactIn = buildSwapExactIn; exports.buildSwitchPriceCurve = buildSwitchPriceCurve; exports.buildSwitchRiskCurve = buildSwitchRiskCurve; exports.buildUpdateBaseSpread = buildUpdateBaseSpread; exports.buildUpdateDeltaStaleness = buildUpdateDeltaStaleness; exports.buildUpdateDrawdownGuard = buildUpdateDrawdownGuard; exports.buildUpdateFactorClamp = buildUpdateFactorClamp; exports.buildUpdateFeeConfig = buildUpdateFeeConfig; exports.buildUpdateMidprice = buildUpdateMidprice; exports.buildUpdateMidpriceAndBaseSpread = buildUpdateMidpriceAndBaseSpread; exports.buildUpdatePoolFeeConfig = buildUpdatePoolFeeConfig; exports.buildUpdateSpreadConfig = buildUpdateSpreadConfig; exports.buildValidateCurveUpdates = buildValidateCurveUpdates; exports.buildWithdraw = buildWithdraw; exports.createWsolAtaInstruction = createWsolAtaInstruction; exports.curvePrefabsSize = curvePrefabsSize; exports.decodeActiveCurves = decodeActiveCurves; exports.decodeConfig = decodeConfig; exports.decodeCurveMeta = decodeCurveMeta; exports.decodeCurveSide = decodeCurveSide; exports.decodeCurveUpdates = decodeCurveUpdates; exports.decodeFeeConfig = decodeFeeConfig; exports.decodeMidpriceOracle = decodeMidpriceOracle; exports.decodeSpreadConfig = decodeSpreadConfig; exports.derivePoolAddresses = derivePoolAddresses; exports.explainIssue = explainIssue; exports.findAssetFeeOverride = findAssetFeeOverride; exports.formatIssue = formatIssue; exports.fromQ32 = fromQ32; exports.getAssetFeeOverrides = getAssetFeeOverrides; exports.getConfigAddress = getConfigAddress; exports.getCurveMetaAddress = getCurveMetaAddress; exports.getCurvePrefabsAddress = getCurvePrefabsAddress; exports.getCurveUpdatesAddress = getCurveUpdatesAddress; exports.getFeeConfigAddress = getFeeConfigAddress; exports.getHadronErrorMessage = getHadronErrorMessage; exports.getMidpriceOracleAddress = getMidpriceOracleAddress; exports.getOrCreateAta = getOrCreateAta; exports.getPoolFeeConfigAddress = getPoolFeeConfigAddress; exports.getSpreadConfigAddress = getSpreadConfigAddress; exports.isNativeMint = isNativeMint; exports.isSlotInitialized = isSlotInitialized; exports.minLegalNextFactor = minLegalNextFactor; exports.pctBaseQ32 = pctBaseQ32; exports.pctToQ32 = pctToQ32; exports.resolveFeePpm = resolveFeePpm; exports.spreadBpsToQ32 = spreadBpsToQ32; exports.spreadQ32ToBps = spreadQ32ToBps; exports.toQ32 = toQ32; exports.unwrapSolInstructions = unwrapSolInstructions; exports.validateCurveConcavity = validateCurveConcavity; exports.validatePriceCurve = validatePriceCurve; exports.validatePriceSegment = validatePriceSegment; exports.validateRiskCurve = validateRiskCurve; exports.validateRiskSegmentParams = validateRiskSegmentParams; exports.wrapSolInstructions = wrapSolInstructions;
//# sourceMappingURL=index.js.map