# Regional statistical close

Produce the 2023 regional close using only the supplied files and local
computation. Inputs are synthetic. Do not change any input file or use external
data. Write the five deliverables below in `/workspace/output/`.

## Sources and rules

- `regions.csv` is the authoritative catalog: `region_code`, `region_name`,
  `state`, `active`. Include every region with `active=1` exactly once. Codes
  are identifiers: preserve their leading zeroes. Names are not join keys.
- `aliases.csv` maps historical `alias` identifiers to canonical `region_code`
  identifiers. Normalize aliases before joining or grouping source records.
- `population.csv` contains `record_id`, `region_code`, `year`, `revision`,
  `published_at`, `status`, `men`, `women`, `population`.
- `income.csv` contains the same first seven columns, followed by `earners`
  and `median_income`. Income is expressed in whole currency units.
- `record_id` is unique within each source. Source file order has no authority.
  The closing date is **2024-06-30**, inclusive. For each source and canonical
  region, select the eligible record with the greatest integer `revision`.
  There are no ties among eligible revisions.
- Classify **every** population and income record with exactly one reason,
  using this precedence: `wrong_year` when year is not 2023; `not_final` when
  status is not `final`; `after_cutoff` when publication is after the closing
  date; `unknown_region` when the normalized code is absent from the catalog;
  `inactive_region` when its catalog region is inactive; `superseded` when an
  eligible record has a greater eligible revision for the same canonical code;
  otherwise `selected`.
- Recover `?` cells only in selected population records, using the exact
  identity `population = men + women`. Every affected selected record has
  exactly one missing component; the other two determine it uniquely. Do not
  substitute zero for missing cells. Count each recovered cell once.
- The selected population and income records must cover the same complete set
  of active regions. Keep regions with zero earners; their income proxy is zero.
- Compute national income quartiles over the selected regions' `median_income`,
  including regions with zero earners. Use nearest-rank thresholds: sort the
  N values ascending, then take positions `ceil(N/4)`, `ceil(N/2)`, and
  `ceil(3*N/4)`, with positions starting at one. Classify as Q1 when income
  is at most the first threshold, Q2 when at most the second, Q3 when at most
  the third, otherwise Q4. Equal incomes must remain in the same quartile.
  Thresholds are national, not recomputed for each state.
- `income_proxy = earners * median_income`. A group's
  `weighted_median_income` means the earners-weighted mean of regional median
  incomes: `sum(income_proxy) / sum(earners)`, not a median or unweighted mean.
  Round once, at the end, to two decimal places using half-up rounding. Use
  zero if the denominator is zero.

## Deliverables

Use CSV headers exactly as listed. Integer fields must be numeric. CSV row
order is unrestricted except for `source.csv`. No extra or duplicate data rows.

1. **`source.csv`**, sorted ascending by canonical region code, with columns:
   `region_code,region_name,state,men,women,population,earners,median_income,quarter,income_proxy,population_record_id,income_record_id`.
   Include one fully reconciled row per active region, canonical catalog names,
   and the exact selected source record IDs.
2. **`states.csv`**, one row per state, with columns:
   `state,regions,population,earners,income_proxy,weighted_median_income`.
3. **`quartiles.csv`**, one row for every state/quarter combination, including
   empty groups, with columns:
   `state,quarter,regions,population,earners,income_proxy,weighted_median_income`.
   Derive these groups from the national quartile labels in `source.csv`.
4. **`audit.csv`**, one row per record in each raw source, with columns:
   `source,record_id,region_code,reason`. Use `population` or `income` for source;
   use the canonical region code after alias normalization, or the original
   code when there is no alias mapping. Include selected and excluded records.
5. **`report.json`**, one object with exactly these fields:
   - `year`: 2023; `cutoff`: `2024-06-30`.
   - `regions`, `population`, `earners`, `income_proxy`: national totals.
   - `weighted_median_income`: the national weighted result rounded as above.
   - `quartile_thresholds`: an object with numeric `Q1`, `Q2`, `Q3` thresholds.
   - `recovered_population_cells`: the count of reconstructed cells.
   - `records`: an object with `population` and `income` objects, each mapping
     all seven audit reasons to their record counts, including zero counts.

All totals must reconcile across source, state, quartile and national outputs.
The source IDs and complete audit must make every selection, exclusion and
recovered population value traceable to the supplied inputs.
