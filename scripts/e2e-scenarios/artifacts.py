"""Shared frozen-fixture persistence and artifact scoring for the hard scenarios."""

import argparse
import csv
import hashlib
import io
import json
import sys
import tarfile
from collections import Counter
from decimal import Decimal, InvalidOperation
from pathlib import Path, PurePosixPath


def digest(content):
    return hashlib.sha256(content).hexdigest()


def csv_bytes(rows):
    stream = io.StringIO(newline="")
    writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return stream.getvalue().encode()


def freeze(root, inputs, expected, contracts):
    """Freeze facts and complete row contracts; generated answers never enter inputs/."""
    (root / "inputs").mkdir(exist_ok=True)
    (root / "fixtures").mkdir(exist_ok=True)
    for name, rows in inputs.items():
        (root / "inputs" / name).write_bytes(csv_bytes(rows))
    content = (json.dumps(expected, indent=2, ensure_ascii=False) + "\n").encode()
    (root / "fixtures/expected.json").write_bytes(content)
    manifest = {
        "scenario": root.name + "-v1",
        "sources": {name: digest((root / name).read_bytes()) for name in
                    ("generate.py", "request.md", "score.py")},
        "scorer_sha256": digest(Path(__file__).read_bytes()),
        "inputs": {name: digest((root / "inputs" / name).read_bytes()) for name in inputs},
        "expected_sha256": digest(content),
        "contracts": contracts,
    }
    (root / "fixtures/manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"scenario": root.name, "inputs": {k: len(v) for k, v in inputs.items()}}))


def number_equal(actual, expected):
    if isinstance(actual, bool) or actual is None:
        return False
    try:
        value = Decimal(str(actual))
        return value.is_finite() and value == Decimal(str(expected))
    except InvalidOperation:
        return False


def compare_json(actual, expected):
    if isinstance(expected, dict):
        return (isinstance(actual, dict) and actual.keys() == expected.keys()
                and all(compare_json(actual[k], v) for k, v in expected.items()))
    if isinstance(expected, list):
        return (isinstance(actual, list) and len(actual) == len(expected)
                and all(compare_json(a, b) for a, b in zip(actual, expected)))
    if isinstance(expected, (int, float)):
        return isinstance(actual, (int, float)) and number_equal(actual, expected)
    return type(actual) is type(expected) and actual == expected


def unique_object(pairs):
    value = dict(pairs)
    if len(value) != len(pairs):
        raise ValueError("Duplicate JSON keys")
    return value


def compare_csv(content, gold, contract):
    reader = csv.DictReader(io.StringIO(content.decode("utf-8-sig"), newline=""))
    if reader.fieldnames != list(gold[0]):
        return ["Header mismatch"]
    rows = list(reader)
    if any(None in r or None in r.values() for r in rows):
        return ["Missing or extra fields"]
    key = lambda r: tuple(str(r[k]) for k in contract["keys"])
    actual = {key(r): r for r in rows}
    expected = {key(r): r for r in gold}
    if len(actual) != len(rows):
        return ["Duplicate row keys"]
    if actual.keys() != expected.keys():
        return ["Row coverage mismatch"]
    errors = []
    for k, row in expected.items():
        if any(not number_equal(actual[k][f], v) if f in contract["numeric"]
               else actual[k][f] != str(v) for f, v in row.items()):
            errors.append("Incorrect row: " + "/".join(k))
    if contract.get("sorted") and [key(r) for r in rows] != sorted(expected):
        errors.append("Row order mismatch")
    return errors


def score(root, run):
    manifest = json.loads((root / "fixtures/manifest.json").read_text())
    for name, expected_hash in manifest["sources"].items():
        if digest((root / name).read_bytes()) != expected_hash:
            raise ValueError("Changed scenario source: " + name)
    if digest(Path(__file__).read_bytes()) != manifest["scorer_sha256"]:
        raise ValueError("Changed shared scorer")
    expected_bytes = (root / "fixtures/expected.json").read_bytes()
    if digest(expected_bytes) != manifest["expected_sha256"]:
        raise ValueError("Changed reference")
    for name, expected_hash in manifest["inputs"].items():
        if digest((root / "inputs" / name).read_bytes()) != expected_hash:
            raise ValueError("Changed frozen input: " + name)
    expected = json.loads(expected_bytes)
    wanted = set(manifest["inputs"]) | {"output/" + name for name in expected}
    files = {}
    archive_path = run / "workspace.tar" if run.is_dir() else run
    with tarfile.open(archive_path) as archive:
        for member in archive:
            name = str(PurePosixPath(member.name))
            if name not in wanted:
                continue
            if name in files or not member.isfile() or member.size > 8 * 1024 * 1024:
                raise ValueError("Duplicate, nonregular or oversized artifact: " + name)
            files[name] = archive.extractfile(member).read()
    checks = [{"file": name, "passed": name in files and digest(files[name]) == value}
              for name, value in manifest["inputs"].items()]
    for name, gold in expected.items():
        try:
            raw = files["output/" + name]
            if name.endswith(".json"):
                actual = json.loads(raw, object_pairs_hook=unique_object)
                errors = [] if compare_json(actual, gold) else ["JSON value/schema mismatch"]
            else:
                errors = compare_csv(raw, gold, manifest["contracts"][name])
        except KeyError:
            errors = ["Missing file"]
        except (ValueError, UnicodeError, csv.Error):
            errors = ["Malformed file"]
        checks.append({"file": "output/" + name, "passed": not errors,
                       "error_count": len(errors), "errors": errors[:5]})
    result = {"scenario": manifest["scenario"], "artifact_pass": all(c["passed"] for c in checks),
              "checks": checks}
    if run.is_dir():
        meta = json.loads((run / "manifest.json").read_text())
        if meta["request"] != (root / "request.md").read_text().strip():
            raise ValueError("Run request differs from the frozen request")
        results_path = run / "results.json"
        usage = (json.loads(results_path.read_text()) if results_path.exists() else meta).get("usage", [])
        costs = Counter()
        missing = 0
        for call in usage:
            cost = (call.get("usage") or {}).get("cost")
            if cost is None:
                missing += 1
            else:
                costs[cost.get("unit", "unspecified")] += cost["amount"]
        result.update(run_id=meta["id"], run_status=meta["status"], config=meta["config"],
                      started_at=meta["startedAt"], completed_at=meta.get("completedAt"),
                      recorded_provider_calls=len(usage), recorded_cost=dict(costs),
                      calls_without_cost=missing)
    return result


def main(root):
    parser = argparse.ArgumentParser(description="Score exported artifacts without executing agent code.")
    parser.add_argument("run", type=Path)
    args = parser.parse_args()
    try:
        result = score(root, args.run)
        print(json.dumps(result, indent=2))
        return 0 if result["artifact_pass"] else 1
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        print(json.dumps({"artifact_pass": False, "scoring_error": str(error)}))
        return 2
