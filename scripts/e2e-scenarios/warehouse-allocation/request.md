# Globally optimal warehouse allocation

Using only the supplied files and local computation, allocate each day's orders
to warehouses. Preserve all input files and write five deliverables under
`/workspace/output/`. Data is synthetic. Identifiers are strings, not numbers.

## Inputs

- `products.csv`: `product,kind`, where kind is `raw` or `kit`.
- `bom.csv`: `parent,component,quantity`. Kits can contain other kits. The BOM is
  acyclic. Recursively expand each kit to raw components and sum every path to a
  shared raw component. Raw products consume one unit of themselves. There is no
  separate stock of kits; assembly is free and unlimited if components exist.
- `aliases.csv`: historical product identifiers to canonical products. Normalize
  order products before expansion. Products have no substitute components.
- `warehouses.csv`: warehouse identifiers. Every warehouse exists every day.
- `inventory.csv`: `day,warehouse,component,quantity` of raw components available
  at the start of each independent day. No carryover, transfers, replenishment,
  reservations or negative stock. Every day×warehouse×raw-component row exists.
- `capacity.csv`: `day,warehouse,max_orders`; limits accepted order count, not
  ordered units or component count.
- `limits.csv`: `day,budget_cents,carbon_limit`. Both limits apply across ALL
  warehouses together on that day.
- `lanes.csv`: `warehouse,zone,transit_days,base_cents,unit_cents,carbon_per_unit`.
  A missing warehouse/zone lane is unavailable. Freight for one accepted order
  is `base_cents + unit_cents * quantity`; carbon is `carbon_per_unit * quantity`.
- `orders.csv`: `day,order_id,product,quantity,zone,max_transit_days,priority_points`.
  Order IDs are unique. Row order never determines priority. The whole quantity
  of an order is either accepted at exactly one warehouse or rejected. No splitting
  an order across warehouses, partial fulfillment or consuming rejected stock.

## Feasibility and global objective

For every accepted order: a lane must exist, its transit time must be at most
`max_transit_days`, and the warehouse must have ALL recursively expanded raw
components for the full order quantity. Across accepted orders, respect each
warehouse's stock and max_orders, and the day's shared freight/carbon limits.

Find the **globally optimal** feasible assignment for EACH independent day using
this lexicographic objective, in order:

1. Maximize total priority_points of accepted orders. Points are per order, not
   multiplied by quantity.
2. Maximize accepted quantity (product units, not raw components).
3. Minimize total freight_cents.
4. Minimize total carbon.
5. Minimize the assignment signature lexicographically: sort that day's orders by
   order_id, encode each as its warehouse ID or `~` if rejected, and join with `|`.
   Warehouse IDs are fixed-width ASCII; `~` sorts after every warehouse ID.

Do not use a greedy approximation. A locally attractive order may prevent a
better combination, and a warehouse with cheaper freight may hold scarce kit
components needed elsewhere. No global optimization across different days is needed.
The instance is small enough for an exact standard-library implementation.

## Deliverables

Use exact headers, no extra/duplicate data rows, and ordinary integer numeric
fields. Row order is unrestricted. JSON keys must be exactly those specified.

1. `bom.csv`: `product,component,quantity_per_unit`. One row for every
   product×raw-component with positive expanded requirement, including raw
   products themselves. Use canonical identifiers.
2. `assignments.csv`:
   `day,order_id,product,quantity,warehouse,accepted,priority_points,freight_cents,carbon`.
   Exactly one row per input order, canonical product and original quantity.
   For rejection, warehouse is an empty cell and accepted/priority_points/
   freight_cents/carbon are zero; for acceptance, accepted is one and points are
   the input order's points. Assignments must attain all five objective levels.
3. `stock.csv`: `day,warehouse,component,opening,used,closing` — every input
   inventory key once, including zero-use and zero-closing rows; used equals
   component consumption from the chosen assignments and opening−used=closing.
4. `daily.csv`:
   `day,orders,accepted_orders,rejected_orders,priority_points,units,freight_cents,carbon,budget_remaining,carbon_remaining,signature`.
   Exactly one row per day, consistent with its optimal assignments and limits.
5. `report.json`: exactly `days`, `orders`, `accepted_orders`, `rejected_orders`,
   `priority_points`, `units`, `freight_cents`, `carbon`, `component_units_used`.
   These are integer totals across all days. component_units_used counts raw
   components consumed, not ordered kit units.

All outputs must agree: BOM expansion drives stock consumption; assignment
costs and points drive daily totals; the report aggregates daily and stock files.
