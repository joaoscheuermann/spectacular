"""Build shared-resource allocation instances and certify their optima by enumeration."""
import random
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from artifacts import freeze

ROOT = Path(__file__).resolve().parent
RAW = ["A", "B", "C", "D"]
WAREHOUSES = ["001", "002", "003"]
# Independent flattened facts specify the public nested recipes' exact expansion.
EXPANSION = {"A": [1, 0, 0, 0], "B": [0, 1, 0, 0], "C": [0, 0, 1, 0], "D": [0, 0, 0, 1],
             "K1": [2, 1, 0, 0], "K2": [1, 0, 2, 0],
             "K3": [3, 1, 2, 1], "K4": [7, 3, 2, 1]}


def optimum(orders, inventories, capacities, lanes, limit):
    """Enumerate every feasible assignment, including rejection; no heuristic cutoff."""
    best = None
    choices = None
    visited = 0

    def visit(index, stock, counts, points, units, cost, carbon, assigned):
        nonlocal best, choices, visited
        if index == len(orders):
            visited += 1
            signature = "|".join(assigned)
            objective = (-points, -units, cost, carbon, signature)
            if best is None or objective < best:
                best, choices = objective, assigned[:]
            return
        order = orders[index]
        quantity = order["quantity"]
        needs = [quantity * n for n in EXPANSION[order["product"].removeprefix("OLD-")]]
        visit(index + 1, stock, counts, points, units, cost, carbon, assigned + ["~"])
        for w in WAREHOUSES:
            lane = lanes.get((w, order["zone"]))
            if lane is None or lane["transit_days"] > order["max_transit_days"]:
                continue
            freight = lane["base_cents"] + quantity * lane["unit_cents"]
            emissions = quantity * lane["carbon_per_unit"]
            if (counts[w] == capacities[w] or cost + freight > limit["budget_cents"]
                    or carbon + emissions > limit["carbon_limit"]
                    or any(a < b for a, b in zip(stock[w], needs))):
                continue
            next_stock = {**stock, w: [a - b for a, b in zip(stock[w], needs)]}
            visit(index + 1, next_stock, {**counts, w: counts[w] + 1},
                  points + order["priority_points"], units + quantity, cost + freight,
                  carbon + emissions, assigned + [w])

    visit(0, inventories, {w: 0 for w in WAREHOUSES}, 0, 0, 0, 0, [])
    if visited < 2:
        raise ValueError("Instance has no meaningful feasible choice")
    return choices, visited


def generate():
    rng = random.Random(48812)
    recipes = [("K1", "A", 2), ("K1", "B", 1), ("K2", "A", 1), ("K2", "C", 2),
               ("K3", "K1", 1), ("K3", "K2", 1), ("K3", "D", 1),
               ("K4", "K1", 2), ("K4", "K3", 1)]
    lanes = [{"warehouse": w, "zone": zone, "transit_days": 1 + (wi + zi) % 3,
              "base_cents": 30 + wi * 20, "unit_cents": 20 + zi * 15 + wi * 5,
              "carbon_per_unit": 2 + wi + zi}
             for wi, w in enumerate(WAREHOUSES) for zi, zone in enumerate(["N", "S", "E"])
             if not (w == "003" and zone == "E")]
    lane_map = {(r["warehouse"], r["zone"]): r for r in lanes}
    inventory, capacity, limits, orders = [], [], [], []
    assignments, stocks, daily = [], [], []
    certificates = []
    for day_index in range(12):
        day = f"2025-04-{day_index + 1:02}"
        opening = {w: [rng.randint(8, 34) for _ in RAW] for w in WAREHOUSES}
        caps = {w: rng.randint(2, 4) for w in WAREHOUSES}
        limit = {"day": day, "budget_cents": 600 + 75 * (day_index % 4),
                 "carbon_limit": 40 + 8 * (day_index % 3)}
        limits.append(limit)
        for w in WAREHOUSES:
            capacity.append({"day": day, "warehouse": w, "max_orders": caps[w]})
            inventory.extend({"day": day, "warehouse": w, "component": c, "quantity": qty}
                             for c, qty in zip(RAW, opening[w]))
        current = [{"day": day, "order_id": f"O{day_index + 1:02}-{i + 1:02}",
                    "product": ("OLD-" if i % 3 == 0 else "") + rng.choice(list(EXPANSION)),
                    "quantity": rng.randint(1, 3), "zone": rng.choice(["N", "S", "E"]),
                    "max_transit_days": rng.randint(1, 3), "priority_points": rng.choice([10, 20, 30, 40])}
                   for i in range(10)]
        # Guarantee nested kits and indivisible contention in every day's instance.
        current[0].update(product="OLD-K4", quantity=2, priority_points=40)
        current[1].update(product="K3", quantity=2, priority_points=30)
        chosen, visited = optimum(current, opening, caps, lane_map, limit)
        certificates.append(visited)
        orders.extend(current)
        used = {w: [0] * len(RAW) for w in WAREHOUSES}
        totals = Counter()
        for order, w in zip(current, chosen):
            product = order["product"].removeprefix("OLD-")
            accepted = w != "~"
            lane = lane_map[(w, order["zone"])] if accepted else None
            freight = lane["base_cents"] + order["quantity"] * lane["unit_cents"] if accepted else 0
            carbon = order["quantity"] * lane["carbon_per_unit"] if accepted else 0
            points = order["priority_points"] if accepted else 0
            assignments.append({"day": day, "order_id": order["order_id"], "product": product,
                                "quantity": order["quantity"], "warehouse": w if accepted else "",
                                "accepted": int(accepted), "priority_points": points,
                                "freight_cents": freight, "carbon": carbon})
            if accepted:
                used[w] = [a + order["quantity"] * b for a, b in zip(used[w], EXPANSION[product])]
            totals.update(accepted_orders=int(accepted), priority_points=points,
                          units=order["quantity"] if accepted else 0, freight_cents=freight, carbon=carbon)
        daily.append({"day": day, "orders": len(current), "accepted_orders": totals["accepted_orders"],
                      "rejected_orders": len(current) - totals["accepted_orders"],
                      **{k: totals[k] for k in ["priority_points", "units", "freight_cents", "carbon"]},
                      "budget_remaining": limit["budget_cents"] - totals["freight_cents"],
                      "carbon_remaining": limit["carbon_limit"] - totals["carbon"],
                      "signature": "|".join(chosen)})
        stocks.extend({"day": day, "warehouse": w, "component": component,
                       "opening": opening[w][i], "used": used[w][i], "closing": opening[w][i] - used[w][i]}
                      for w in WAREHOUSES for i, component in enumerate(RAW))
    expanded = [{"product": p, "component": c, "quantity_per_unit": q}
                for p, values in EXPANSION.items() for c, q in zip(RAW, values) if q]
    report = {"days": len(daily), **{k: sum(r[k] for r in daily) for k in
              ["orders", "accepted_orders", "rejected_orders", "priority_points", "units", "freight_cents", "carbon"]},
              "component_units_used": sum(r["used"] for r in stocks)}
    inputs = {"products.csv": [{"product": p, "kind": "raw" if p in RAW else "kit"} for p in EXPANSION],
              "bom.csv": [{"parent": p, "component": c, "quantity": q} for p, c, q in recipes],
              "aliases.csv": [{"alias": "OLD-" + p, "product": p} for p in EXPANSION],
              "warehouses.csv": [{"warehouse": w} for w in WAREHOUSES], "inventory.csv": inventory,
              "capacity.csv": capacity, "limits.csv": limits, "lanes.csv": lanes, "orders.csv": orders}
    for rows in inputs.values():
        rng.shuffle(rows)
    expected = {"bom.csv": expanded, "assignments.csv": assignments,
                "stock.csv": stocks, "daily.csv": daily, "report.json": report}
    keys = {"bom.csv": ["product", "component"], "assignments.csv": ["order_id"],
            "stock.csv": ["day", "warehouse", "component"], "daily.csv": ["day"]}
    text_fields = {"day", "order_id", "warehouse", "component", "product", "signature"}
    contracts = {name: {"keys": key, "numeric": [f for f in expected[name][0] if f not in text_fields]}
                 for name, key in keys.items()}
    freeze(ROOT, inputs, expected, contracts)
    print({"feasible_assignments_enumerated_per_day": certificates})


if __name__ == "__main__":
    generate()
