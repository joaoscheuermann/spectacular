# Inventory replay

Twenty-four SKU histories are interleaved across three warehouses. Each includes
alias normalization, noisy revisions, future knowledge, cancellation, FIFO
consumption, a return spanning two lots, a multi-lot transfer, insufficient stock
and invalid return references.

The rejected transfer must leave both warehouses unchanged. The return restores
an exhausted old receipt, so treating it as a new-age lot changes later FIFO
allocations. Expected fragments and closing balances are authored directly from
canonical construction facts, independently of event selection and replay.

The case has 768 raw event rows, 144 applied events, 288 allocation rows, 48
positive closing lots and 72 warehouse/SKU balances. All five outputs are scored,
including zero balances, exclusion precedence, lot provenance and integer costs.

Use [the paired commands](../README.md) with `SCENARIO=inventory-replay`.
Supply only `inputs/`, never the scenario root.
