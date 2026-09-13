"""Generate deterministic inputs and private scoring references from canonical facts."""

import csv
import hashlib
import io
import json
import random
from collections import Counter
from decimal import ROUND_HALF_UP, Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parent
STATES = ("Aurora", "Boreal", "Cedro", "Duna", "Estrela", "Foz", "Granito", "Horizonte")
REASONS = (
    "wrong_year", "not_final", "after_cutoff", "unknown_region",
    "inactive_region", "superseded", "selected",
)


def csv_bytes(rows):
    buffer = io.StringIO(newline="")
    writer = csv.DictWriter(buffer, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return buffer.getvalue().encode()


def canonical_regions():
    """Author complete facts before making ambiguous source presentations."""
    regions = []
    for index in range(240):
        state, local = divmod(index, 30)
        men = 2000 + (index * 97) % 7000
        women = 2100 + (index * 139) % 7000
        regions.append({
            "region_code": f"{state + 1:02}{local + 1:04}",
            "region_name": f"Distrito {local + 1:02}",
            "state": STATES[state],
            "men": men, "women": women, "population": men + women,
            "earners": 0 if index % 47 == 0 else 500 + (index * 83) % 4000,
            "median_income": 20000 + state * 4500 + (index * 37 % 29) * 500,
        })
    return regions


def record(source, region, index):
    code = region["region_code"]
    common = {
        "record_id": f"{source[0].upper()}{index:04}-current",
        "region_code": "LEG-" + code if index % 8 == 0 else code,
        "year": 2023, "revision": 2, "published_at": "2024-06-15", "status": "final",
    }
    fields = ("men", "women", "population") if source == "population" else ("earners", "median_income")
    return {**common, **{field: region[field] for field in fields}}


def variants(current, index):
    """Assign exclusion reasons at construction, independently of a source selector."""
    rows = [("wrong_year", {**current, "year": 2022, "revision": 99})]
    if index % 4 == 0:
        older = {**current, "revision": 1, "published_at": "2024-01-01"}
        field = "men" if "men" in older else "median_income"
        older[field] = 999999
        rows.append(("superseded", older))
    if index % 10 == 0:
        rows.extend([
            ("not_final", {**current, "revision": 8, "status": "provisional"}),
            ("after_cutoff", {**current, "revision": 9, "published_at": "2024-07-01"}),
        ])
    return rows


def source_records(source, regions):
    rows, audit = [], []
    for index, region in enumerate(regions):
        current = record(source, region, index)
        alternatives = variants(current, index)
        if source == "population" and index % 15 < 3:
            current[("men", "women", "population")[index % 15]] = "?"
        for reason, row in [("selected", current), *alternatives]:
            suffix = "current" if reason == "selected" else reason
            row = {**row, "record_id": f"{source[0].upper()}{index:04}-{suffix}"}
            rows.append(row)
            audit.append({"source": source, "record_id": row["record_id"],
                          "region_code": region["region_code"], "reason": reason})
    return rows, audit


def add_outside_records(source, data):
    rows, audit = data
    for index in range(8):
        for reason, prefix in (("unknown_region", "88"), ("inactive_region", "99")):
            code = f"{prefix}{index + 1:04}"
            row = {**rows[0], "record_id": f"{source[0]}-{code}", "region_code": code}
            rows.append(row)
            audit.append({"source": source, "record_id": row["record_id"],
                          "region_code": code, "reason": reason})
    random.Random(731 if source == "population" else 947).shuffle(rows)
    return rows, audit


def source_table(regions):
    incomes = sorted(region["median_income"] for region in regions)
    thresholds = [incomes[position - 1] for position in (60, 120, 180)]
    rows = []
    for index, region in enumerate(regions):
        quarter = 1 + sum(region["median_income"] > value for value in thresholds)
        rows.append({
            **region, "quarter": f"Q{quarter}",
            "income_proxy": region["earners"] * region["median_income"],
            "population_record_id": f"P{index:04}-current",
            "income_record_id": f"I{index:04}-current",
        })
    return rows, dict(zip(("Q1", "Q2", "Q3"), thresholds))


def totals(rows):
    sums = {field: sum(row[field] for row in rows)
            for field in ("population", "earners", "income_proxy")}
    weighted = Decimal(sums["income_proxy"]) / sums["earners"] if sums["earners"] else Decimal(0)
    return {"regions": len(rows), **sums,
            "weighted_median_income": float(weighted.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))}


def expected_outputs(regions, audits):
    source, thresholds = source_table(regions)
    states, quartiles = [], []
    for state in STATES:
        group = [row for row in source if row["state"] == state]
        states.append({"state": state, **totals(group)})
        for quarter in ("Q1", "Q2", "Q3", "Q4"):
            subset = [row for row in group if row["quarter"] == quarter]
            quartiles.append({"state": state, "quarter": quarter, **totals(subset)})
    counts = {name: Counter(row["reason"] for row in rows) for name, rows in audits.items()}
    report = {"year": 2023, "cutoff": "2024-06-30", **totals(source),
              "quartile_thresholds": thresholds, "recovered_population_cells": 48,
              "records": {name: {reason: count[reason] for reason in REASONS}
                          for name, count in counts.items()}}
    return {"source.csv": source, "states.csv": states, "quartiles.csv": quartiles,
            "audit.csv": audits["population"] + audits["income"], "report.json": report}


def input_tables(regions):
    catalog = [{**{key: row[key] for key in ("region_code", "region_name", "state")}, "active": 1}
               for row in regions]
    catalog.extend({"region_code": f"99{index + 1:04}", "region_name": "Retired",
                    "state": state, "active": 0} for index, state in enumerate(STATES))
    aliases = [{"alias": "LEG-" + row["region_code"], "region_code": row["region_code"]}
               for index, row in enumerate(regions) if index % 8 == 0]
    sources = {name: add_outside_records(name, source_records(name, regions))
               for name in ("population", "income")}
    tables = {"regions.csv": catalog, "aliases.csv": aliases,
              **{name + ".csv": rows for name, (rows, _) in sources.items()}}
    return tables, {name: audit for name, (_, audit) in sources.items()}


def main():
    regions = canonical_regions()
    tables, audits = input_tables(regions)
    inputs = ROOT / "inputs"
    reference = ROOT / "fixtures"
    inputs.mkdir(exist_ok=True)
    reference.mkdir(exist_ok=True)
    for name, rows in tables.items():
        (inputs / name).write_bytes(csv_bytes(rows))
    expected = expected_outputs(regions, audits)
    expected_bytes = (json.dumps(expected, ensure_ascii=False, indent=2) + "\n").encode()
    (reference / "expected.json").write_bytes(expected_bytes)
    manifest = {"version": 1, "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "request_sha256": hashlib.sha256((ROOT / "request.md").read_bytes()).hexdigest(),
                "inputs": {name: hashlib.sha256((inputs / name).read_bytes()).hexdigest() for name in tables},
                "expected_sha256": hashlib.sha256(expected_bytes).hexdigest()}
    (reference / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"regions": len(regions), "source_records": {key: len(value) for key, value in audits.items()}}))


if __name__ == "__main__":
    main()
