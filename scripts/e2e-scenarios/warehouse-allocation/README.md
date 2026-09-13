# Warehouse allocation

This task combines recursive kit expansion, indivisible orders, scarce shared
components, warehouse capacity, delivery eligibility, and daily freight/carbon
limits. A five-level lexicographic objective determines the unique optimum.

Twelve independent days contain ten orders each. The private oracle enumerates
every feasible assignment, including rejection, with no heuristic or time cutoff.
Flattened kit facts are specified independently of the public nested recipes.

Five outputs demonstrate the solution: BOM requirements, order decisions,
component consumption, daily objective values/signatures and global totals.
A feasible but suboptimal allocation fails. The case is small enough for exact
standard-library computation; no package installation or external optimizer is needed.
Its actual difficulty for the agents has not yet been measured.

Use [the paired commands](../README.md) with `SCENARIO=warehouse-allocation`.
Supply only `inputs/`, never the scenario root.
