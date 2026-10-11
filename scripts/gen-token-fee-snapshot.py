#!/usr/bin/env python3
"""Validate a complete finalized mint scan and produce deterministic fee seeds."""
import argparse
import base64
import hashlib
import json
import re
import struct
from pathlib import Path

PROGRAM = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
MAX_U64 = (1 << 64) - 1


def public_key(value):
    if not isinstance(value, str) or not value or len(value) > 44:
        raise ValueError("invalid mint public key")
    number = 0
    for char in value:
        number = number * 58 + ALPHABET.index(char)
    prefix = len(value) - len(value.lstrip("1"))
    decoded = bytes(prefix) + (number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b"")
    if len(decoded) != 32:
        raise ValueError("invalid mint public key length")
    return decoded


def integer(value, maximum=MAX_U64):
    if isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]*", value):
        value = int(value)
    if type(value) is not int or not 0 <= value <= maximum:
        raise ValueError("invalid unsigned integer")
    return value


def generate(directory, epoch_proof, output):
    if not (directory / "proof.json").exists():
        raise ValueError("snapshot is incomplete: completion proof missing")
    proof = json.loads((directory / "proof.json").read_text())
    if proof.get("complete") is not True or proof.get("program") != PROGRAM:
        raise ValueError("snapshot is incomplete or has the wrong owner")
    if "paginationKey" in proof:
        raise ValueError("pagination state does not belong in release proof")
    expected_filter = [{"memcmp": {"offset": 82, "bytes": "1" * 83}}]
    if proof.get("filters") != expected_filter or proof.get("accountTypes") != [0, 1]:
        raise ValueError("snapshot must cover initialized and pending fee mints together")
    anchor = integer(proof["anchorSlot"])
    minimum = integer(proof["minimumContext"])
    maximum = integer(proof["maximumContext"])
    count = integer(proof["fees"])
    scanned = integer(proof["scanned"])
    initialized = integer(proof["initialized"])
    pages = integer(proof["pages"])
    if not anchor <= minimum <= maximum or not 0 < count <= scanned or initialized > scanned or pages == 0:
        raise ValueError("invalid snapshot count or read window")
    if not re.fullmatch(r"[0-9a-f]{64}", proof.get("ownershipDigest", "")):
        raise ValueError("missing owned-scan integrity digest")
    original = (directory / "fees.jsonl").read_bytes()
    if hashlib.sha256(original).hexdigest() != proof["feeRecordsSha256"]:
        raise ValueError("fee record integrity mismatch")
    epoch_response = json.loads(epoch_proof.read_text())
    epoch_account = epoch_response["value"][2]
    epoch_data = base64.b64decode(epoch_account["data"][0], validate=True)
    if (epoch_account["owner"] != "Sysvar1111111111111111111111111111111111111"
            or epoch_account["executable"] is not False
            or epoch_data != struct.pack("<QQBQQ", 432_000, 432_000, 0, 0, 0)):
        raise ValueError("unsupported or unowned epoch schedule")
    seen = set()
    records = []
    for line in original.splitlines():
        row = json.loads(line)
        mint = public_key(row["mint"])
        context = integer(row["contextSlot"])
        if mint in seen or not minimum <= context <= maximum or row["owner"] != PROGRAM or type(row["initialized"]) is not bool:
            raise ValueError("duplicate mint, invalid owner or invalid read context")
        seen.add(mint)
        data = base64.b64decode(row["mintDataBase64"], validate=True)
        if (len(data) < 166 or len(data) == 355 or data[165] not in (0, 1)
                or data[45] != data[165] or bool(data[45]) != row["initialized"]
                or any(data[82:165]) or struct.unpack_from("<I", data, 0)[0] not in (0, 1)
                or struct.unpack_from("<I", data, 46)[0] not in (0, 1)):
            raise ValueError("invalid owned mint base layout")
        offset = 166
        fee_bytes = None
        while offset < len(data):
            if not any(data[offset:]):
                break
            if offset + 4 > len(data):
                raise ValueError("truncated extension header")
            kind, size = struct.unpack_from("<HH", data, offset)
            offset += 4
            if kind == 0 or offset + size > len(data):
                raise ValueError("invalid extension bounds")
            if kind == 1:
                if fee_bytes is not None or size != 108:
                    raise ValueError("invalid or duplicate fee extension")
                fee_bytes = data[offset + 72:offset + 108]
            offset += size
        if fee_bytes is None:
            raise ValueError("record does not contain a fee configuration")
        fees = []
        for index, name in enumerate(("older", "newer")):
            epoch, cap, bps = struct.unpack_from("<QQH", fee_bytes, index * 18)
            declared = row[name]
            expected = (integer(declared["epoch"]), integer(declared["maximumFee"]), integer(declared["basisPoints"], 10_000))
            if (epoch, cap, bps) != expected or bps > 10_000 or epoch > context // 432_000 + 2:
                raise ValueError("fee configuration disagrees with owned mint bytes")
            fees.append(epoch)
        if fees[0] > fees[1] or fees[0] > context // 432_000:
            raise ValueError("invalid fee epoch ordering")
        records.append((context, mint, struct.pack("<Q", context) + mint + fee_bytes + bytes([data[45], 0, 0, 0])))
    if len(records) != count:
        raise ValueError("snapshot record count mismatch")
    records.sort(key=lambda item: (item[0], item[1]))
    seed = b"".join(item[2] for item in records)
    if len(seed) != count * 80:
        raise ValueError("invalid compact seed size")
    output.mkdir(parents=True, exist_ok=True)
    release_proof = {**proof, "seedRecords": count, "seedSha256": hashlib.sha256(seed).hexdigest(),
                     "seedRecordSize": 80, "epochSchedule": {"address": "SysvarEpochSchedu1e111111111111111111111111",
                     "owner": epoch_account["owner"], "contextSlot": integer(epoch_response["context"]["slot"]),
                     "dataBase64": epoch_account["data"][0]}, "pendingFeeMints": sum(not line[2][76] for line in records)}
    # All validation precedes writes; incomplete scans never produce a seed.
    (output / "mint-fees.bin").write_bytes(seed)
    (output / "proof.json").write_text(json.dumps(release_proof, indent=2) + "\n")
    (output / "snapshot.rs").write_text(
        "// Generated only from a complete, owner-verified finalized scan.\n"
        f"pub(crate) const ANCHOR: u64 = {anchor};\n"
        f"pub(crate) const COMPLETE_AT: u64 = {maximum};\n"
        f"pub(crate) const COUNT: i64 = {count};\n"
        f'pub(crate) const SEED: &[u8; {len(seed)}] = include_bytes!("mint-fees.bin");\n')
    return {"anchor": anchor, "completeAt": maximum, "records": count, "bytes": len(seed), "sha256": release_proof["seedSha256"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--epoch-proof", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        print(json.dumps(generate(arguments.directory, arguments.epoch_proof, arguments.output)))
    except (OSError, ValueError, KeyError, TypeError, struct.error) as error:
        raise SystemExit("snapshot generation failed: " + str(error)) from None
