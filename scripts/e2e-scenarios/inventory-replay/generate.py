"""Author closed-form stock/lot facts, then hide them behind shuffled event revisions."""
import random
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from artifacts import freeze

ROOT = Path(__file__).resolve().parent
REASONS = ["late_received", "superseded", "cancelled", "after_close", "unknown_warehouse",
           "unknown_sku", "inactive_sku", "invalid_return", "insufficient_stock", "applied"]


def generate():
    events, audit, allocations, lots, stock = [], [], [], [], []
    products = [{"sku": f"{i:05}", "active": 1} for i in range(1, 25)]
    products.append({"sku": "99999", "active": 0})
    aliases = [{"alias": "OLD-" + p["sku"], "sku": p["sku"]} for p in products[:-1]]
    warehouses = [{"warehouse": w} for w in ["001", "002", "003"]]
    totals = Counter()

    def raw(event, reason, **changes):
        row = {**event, **changes}
        row["record_id"] = f"{row['event_id']}-r{row['revision']}"
        events.append(row)
        audit.append({"record_id": row["record_id"], "event_id": row["event_id"],
                      "sku": row["sku"].removeprefix("OLD-"), "reason": reason})

    for index, product in enumerate(products[:-1]):
        sku = product["sku"]
        n, c = 10 + index % 3, 100 + 7 * index
        d = c + 40
        prefix = f"E{index + 1:03}"
        first, second, issue = prefix + "-01", prefix + "-02", prefix + "-03"
        selected = []
        specs = [
            ("RECEIPT", "001", "", n, c, "", "applied"),
            ("RECEIPT", "001", "", 6, d, "", "applied"),
            ("ISSUE", "001", "", n + 2, 0, "", "applied"),
            ("RETURN", "001", "", 3, 9999, issue, "applied"),
            ("TRANSFER", "001", "002", 5, 0, "", "applied"),
            ("ISSUE", "002", "", 3, 0, "", "applied"),
            ("TRANSFER", "001", "002", 99, 0, "", "insufficient_stock"),
            ("RETURN", "001", "", n + 2, 0, issue, "invalid_return"),
            ("RECEIPT", "001", "", 999, c, "", "cancelled"),
        ]
        for step, (kind, warehouse, destination, quantity, cost, reference, reason) in enumerate(specs, 1):
            event = {"record_id": "", "event_id": f"{prefix}-{step:02}", "revision": 2,
                     "received_at": "2025-03-05", "effective_at": f"2025-02-{step:02}",
                     "cancelled": int(reason == "cancelled"), "kind": kind,
                     "warehouse": warehouse, "destination": destination,
                     "sku": "OLD-" + sku if step % 2 else sku,
                     "quantity": quantity, "unit_cost": cost, "reference_id": reference}
            selected.append(event)
            raw(event, reason)
            raw(event, "superseded", revision=1, quantity=777, cancelled=0)
            raw(event, "late_received", revision=9, received_at="2025-03-06", cancelled=1)

        def allocation(step, leg, fragments, warehouse):
            for pos, (lot, qty, unit) in enumerate(fragments, 1):
                allocations.append({"event_id": f"{prefix}-{step:02}", "leg": leg,
                                    "fragment": pos, "lot_id": lot, "warehouse": warehouse,
                                    "sku": sku, "quantity": qty, "unit_cost": unit, "value": qty * unit})

        # Closed-form allocations are authored from this construction, not replayed from events.csv.
        allocation(1, "receipt", [(first, n, c)], "001")
        allocation(2, "receipt", [(second, 6, d)], "001")
        allocation(3, "issue", [(first, n, c), (second, 2, d)], "001")
        allocation(4, "return", [(second, 2, d), (first, 1, c)], "001")
        allocation(5, "transfer_out", [(first, 1, c), (second, 4, d)], "001")
        allocation(5, "transfer_in", [(first, 1, c), (second, 4, d)], "002")
        allocation(6, "issue", [(first, 1, c), (second, 2, d)], "002")
        for warehouse in ["001", "002"]:
            lots.append({"warehouse": warehouse, "sku": sku, "lot_id": second,
                         "quantity": 2, "unit_cost": d, "value": 2 * d})
        for warehouse in ["001", "002", "003"]:
            qty = 0 if warehouse == "003" else 2
            stock.append({"warehouse": warehouse, "sku": sku, "quantity": qty, "value": qty * d})
        totals.update(received_units=n + 6, issued_units=n + 5, returned_units=3,
                      transferred_units=5, closing_units=4, received_value=n * c + 6 * d,
                      net_cogs=n * c + 2 * d, closing_value=4 * d)
        for offset, reason, changes in [
            (10, "after_close", {"effective_at": "2025-03-01", "sku": "MISSING"}),
            (11, "unknown_warehouse", {"warehouse": "BAD", "sku": "MISSING"}),
            (12, "unknown_sku", {"sku": "MISSING"}),
            (13, "inactive_sku", {"sku": "99999"}),
            (14, "invalid_return", {"kind": "RETURN", "reference_id": "NO-SUCH-ISSUE"}),
        ]:
            raw({**selected[0], "event_id": f"{prefix}-{offset}", "effective_at": "2025-02-28",
                 **changes}, reason)
    random.Random(9031).shuffle(events)
    counts = Counter(r["reason"] for r in audit)
    report = {"effective_cutoff": "2025-02-28", "received_cutoff": "2025-03-05",
              "active_skus": 24, "warehouses": 3, **dict(totals),
              "audit_reasons": {r: counts[r] for r in REASONS}}
    expected = {"audit.csv": audit, "allocations.csv": allocations, "lots.csv": lots,
                "stock.csv": stock, "report.json": report}
    contracts = {
        "audit.csv": {"keys": ["record_id"], "numeric": []},
        "allocations.csv": {"keys": ["event_id", "leg", "fragment"],
                            "numeric": ["fragment", "quantity", "unit_cost", "value"]},
        "lots.csv": {"keys": ["warehouse", "sku", "lot_id"], "numeric": ["quantity", "unit_cost", "value"]},
        "stock.csv": {"keys": ["warehouse", "sku"], "numeric": ["quantity", "value"]},
    }
    freeze(ROOT, {"products.csv": products, "warehouses.csv": warehouses,
                  "aliases.csv": aliases, "events.csv": events}, expected, contracts)


if __name__ == "__main__":
    generate()
