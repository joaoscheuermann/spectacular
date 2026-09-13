# Auditable inventory close

Reconstruct the inventory close as of effective date **2025-02-28**, using only
information received by **2025-03-05** inclusive. Use the supplied files and local
computation. Preserve every input byte. Write the five deliverables to
`/workspace/output/`. No external data or additional dependencies are needed.

## Inputs and selection

- `warehouses.csv`: authoritative warehouse identifiers.
- `products.csv`: authoritative SKU identifiers and `active` flags.
- `aliases.csv`: historical SKU aliases; normalize before any lookup or grouping.
- `events.csv`: unordered event revisions. `record_id` uniquely identifies a raw
  row; `event_id` identifies a logical event. All dates are ISO dates. All amounts
  and unit costs are integer cents. Identifiers are strings; preserve zeroes.
- Only rows with `received_at <= 2025-03-05` participate in revision selection.
  Select the greatest integer `revision` for each event ID among those rows.
  Revisions replace the whole event, including cancellation. There are no ties.
  A later-received revision cannot supersede an earlier eligible revision.
- Replay selected events by `(effective_at, event_id)` ascending, independent
  of file order and received date. There is no opening inventory.

Classify every raw row with exactly one audit reason, in this precedence:
`late_received`; `superseded`; `cancelled`; `after_close`; `unknown_warehouse`;
`unknown_sku`; `inactive_sku`; `invalid_return`; `insufficient_stock`; `applied`.
The first two reasons concern selection; all later reasons apply only to the
selected revision. A selected cancellation never revives a previous revision.
For a transfer, both warehouse and destination must be known. For other events,
only warehouse is checked. Empty destination/reference fields are intentional.
Only RETURN uses `reference_id`; `invalid_return` is explained below.

## Replay rules

All quantities are positive integers. An event either applies fully or has no
effect. Rejected events never create inventory or allocations. Do not fall back
to an older revision after rejection. The four kinds are:

- **RECEIPT:** create a lot with ID equal to this event's ID, at its warehouse,
  with its quantity and `unit_cost`. Lot age is this receipt's replay key.
- **ISSUE:** consume available lots for that SKU at the warehouse in FIFO order
  by the original receipt's replay key. If total stock is insufficient, reject
  the entire event. Cost of goods sold is the sum of consumed lot values.
- **TRANSFER:** consume as for ISSUE and add the exact fragments to the destination.
  Preserve original lot ID, age and unit cost. Both legs are atomic. Transfers
  create no sales or cost of goods sold. A lot may exist in multiple warehouses.
- **RETURN:** reference an earlier applied ISSUE of the same canonical SKU and
  warehouse. Undo its consumption in reverse fragment order, accounting for any
  earlier returns to that issue. Restore the original lot IDs, costs and FIFO ages
  at the original warehouse; coalesce balances with the same lot ID. Reject with
  `invalid_return` if the reference is absent, not yet applied, not an ISSUE,
  mismatched, or the quantity exceeds the issue quantity not yet returned.
  Returns subtract their restored value from cost of goods sold. Return unit_cost
  in the raw row is ignored, as are ISSUE/TRANSFER unit_cost fields.

For each event/leg, number fragments from one in consumption or restoration
order. A RECEIPT has one fragment. TRANSFER incoming fragments mirror outgoing
fragments with the same numbering. RETURN numbering follows the reverse ISSUE
fragment traversal, not FIFO order. All allocation quantities/values are positive;
the leg defines their effect. No zero fragments.

## Deliverables

Use exact headers and fields; integer fields must be numeric. No duplicate or
extra rows. Row order is unrestricted. JSON numbers must be numbers, not strings.

1. `audit.csv`: `record_id,event_id,sku,reason` — one row per raw record, with
   canonical SKU (or the original SKU if there is no alias).
2. `allocations.csv`:
   `event_id,leg,fragment,lot_id,warehouse,sku,quantity,unit_cost,value`.
   Include all applied receipt, issue, return and transfer fragments. Leg values
   are `receipt`, `issue`, `return`, `transfer_out`, `transfer_in`.
3. `lots.csv`: `warehouse,sku,lot_id,quantity,unit_cost,value` — each positive
   closing lot balance exactly once; omit exhausted lots.
4. `stock.csv`: `warehouse,sku,quantity,value` — every warehouse × active SKU
   combination exactly once, including zero balances.
5. `report.json`: exactly `effective_cutoff`, `received_cutoff`, `active_skus`,
   `warehouses`, `received_units`, `issued_units`, `returned_units`,
   `transferred_units`, `closing_units`, `received_value`, `net_cogs`,
   `closing_value`, `audit_reasons`. Count transfer units once, not per leg.
   `audit_reasons` maps all ten reasons above to integer counts, including zeroes.

Reconcile: received − issued + returned = closing units; received value − net
COGS = closing value. Allocations must explain both every applied event and every
closing lot; rejecting an impossible transfer must leave both warehouses unchanged.
