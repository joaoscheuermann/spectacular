# Blinded semantic review protocol

`prepareReview` hashes case IDs with the frozen seed, takes exactly one fifth
of every pre-registered cell, chooses one repetition deterministically, and
emits both M1 and the frozen baseline under opaque condition and run codes.
It derives each public artifact path from the supplied content hash; the
packager must materialize that hash at the derived path. Keep the returned
unblinding key inaccessible while reviewing.

The same solo researcher evaluates every assignment twice. Pass two is not
available until 14 complete days after pass one, and ingestion also verifies
the actual submission interval. The reviewer records criterion decisions,
overall acceptability, confidence from 1 to 5, and optional notes without
seeing condition IDs.

Test-retest stability is Cohen's kappa over paired acceptability
decisions. `openai/gpt-5.6-sol` may be attached only as an auxiliary structured
verdict. It never replaces a human review. If human stability is below 0.80,
or is not estimable because decisions have no variation, the auxiliary judge
must not be extrapolated beyond the reviewed subset.
