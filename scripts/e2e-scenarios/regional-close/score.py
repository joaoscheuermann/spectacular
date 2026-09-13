"""Score exported task artifacts against frozen facts without executing agent code."""

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

ROOT = Path(__file__).resolve().parent
TEXT_FIELDS = {"region_code", "region_name", "state", "quarter", "population_record_id",
               "income_record_id", "source", "record_id", "reason"}
KEYS = {"source.csv": ("region_code",), "states.csv": ("state",),
        "quartiles.csv": ("state", "quarter"), "audit.csv": ("source", "record_id")}


def load_reference():
    """Refuse drift in either the public task or its private reference."""
    manifest = json.loads((ROOT / "fixtures/manifest.json").read_text())
    expected_bytes = (ROOT / "fixtures/expected.json").read_bytes()
    files = {"generate.py": manifest["generator_sha256"], "request.md": manifest["request_sha256"],
             **{"inputs/" + name: digest for name, digest in manifest["inputs"].items()}}
    for name, digest in files.items():
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != digest:
            raise ValueError("Frozen task identity changed: " + name)
    if hashlib.sha256(expected_bytes).hexdigest() != manifest["expected_sha256"]:
        raise ValueError("Frozen expected output identity changed")
    return manifest, json.loads(expected_bytes)


def read_artifacts(path, wanted):
    """Read only task files in memory; never extract links or execute archive contents."""
    result = {}
    with tarfile.open(path) as archive:
        for member in archive:
            name = str(PurePosixPath(member.name))
            if name not in wanted:
                continue
            if name in result or not member.isfile() or member.size > 8 * 1024 * 1024:
                raise ValueError("Duplicate, nonregular or oversized artifact: " + name)
            result[name] = archive.extractfile(member).read()
    return result


def equal_number(actual, expected):
    if isinstance(actual, bool) or actual is None:
        return False
    try:
        value = Decimal(str(actual))
        return value.is_finite() and value == Decimal(str(expected))
    except InvalidOperation:
        return False


def compare_row(actual, expected):
    return all(actual[field] == str(value) if field in TEXT_FIELDS
               else equal_number(actual[field], value) for field, value in expected.items())


def compare_csv(content, name, expected):
    reader = csv.DictReader(io.StringIO(content.decode("utf-8-sig"), newline=""))
    if reader.fieldnames != list(expected[0]):
        return ["CSV header differs from the requested columns or order"]
    rows = list(reader)
    if any(None in row or any(value is None for value in row.values()) for row in rows):
        return ["CSV contains missing or extra fields"]
    keys = KEYS[name]
    keyed = {tuple(row[key] for key in keys): row for row in rows}
    if len(keyed) != len(rows):
        return ["Duplicate row keys"]
    gold = {tuple(str(row[key]) for key in keys): row for row in expected}
    if keyed.keys() != gold.keys():
        return [f"Wrong row coverage: missing={len(gold.keys() - keyed.keys())}, extra={len(keyed.keys() - gold.keys())}"]
    errors = ["Incorrect values for " + "/".join(key) for key in gold
              if not compare_row(keyed[key], gold[key])]
    if name == "source.csv" and [row["region_code"] for row in rows] != sorted(key[0] for key in gold):
        errors.append("source.csv is not sorted by canonical region code")
    return errors


def compare_json(actual, expected, path="report"):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return [path + " has missing or extra fields"]
        return [error for key in expected
                for error in compare_json(actual[key], expected[key], path + "." + key)]
    matches = actual == expected if path.endswith(".cutoff") else isinstance(actual, (int, float)) and equal_number(actual, expected)
    return [] if matches else [path + " differs from the expected value"]


def unique_object(pairs):
    result = dict(pairs)
    if len(result) != len(pairs):
        raise ValueError("Duplicate JSON fields")
    return result


def score_outputs(artifacts, expected):
    checks = []
    for name, gold in expected.items():
        content = artifacts.get("output/" + name)
        if content is None:
            errors = ["Missing required output"]
        else:
            try:
                errors = compare_json(json.loads(content, object_pairs_hook=unique_object), gold) if name.endswith(".json") else compare_csv(content, name, gold)
            except (ValueError, UnicodeError, csv.Error):
                errors = ["Malformed output"]
        checks.append({"file": "output/" + name, "passed": not errors,
                       "errors": errors[:5], "error_count": len(errors)})
    return checks


def run_summary(directory):
    if directory is None:
        return {}
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest["request"] != (ROOT / "request.md").read_text().strip():
        raise ValueError("Run request does not match this frozen scenario")
    results_path = directory / "results.json"
    results = json.loads(results_path.read_text()) if results_path.exists() else manifest
    usage = results.get("usage", [])
    costs = Counter()
    missing_cost = 0
    for call in usage:
        cost = (call.get("usage") or {}).get("cost")
        if cost is None:
            missing_cost += 1
        else:
            costs[cost.get("unit", "unspecified")] += cost["amount"]
    return {"run_id": manifest["id"], "run_status": manifest["status"],
            "started_at": manifest["startedAt"], "completed_at": manifest.get("completedAt"),
            "config": manifest["config"], "recorded_provider_calls": len(usage),
            "recorded_cost": dict(costs), "calls_without_cost": missing_cost}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=Path, help="Run output directory or workspace.tar")
    options = parser.parse_args()
    manifest, expected = load_reference()
    directory = options.run if options.run.is_dir() else None
    archive = directory / "workspace.tar" if directory else options.run
    wanted = set(manifest["inputs"]) | {"output/" + name for name in expected}
    artifacts = read_artifacts(archive, wanted)
    integrity = [{"file": name, "passed": name in artifacts and hashlib.sha256(artifacts[name]).hexdigest() == digest}
                 for name, digest in manifest["inputs"].items()]
    checks = integrity + score_outputs(artifacts, expected)
    result = {"scenario": "regional-close-v1", "artifact_pass": all(check["passed"] for check in checks),
              "checks": checks, **run_summary(directory)}
    print(json.dumps(result, indent=2))
    return 0 if result["artifact_pass"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        print(json.dumps({"artifact_pass": False, "scoring_error": str(error)}))
        sys.exit(2)
