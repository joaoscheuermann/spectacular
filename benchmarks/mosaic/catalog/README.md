# Frozen pilot micro-skill catalog

The canonical catalog is the immutable `SKILLS` value in
`src/catalog/skills.ts`. It contains 60 English micro-skills: 15 each for
documents/finance, software, artifacts, and communication.

Every entry carries a procedural body, an exact deterministic-tool allowlist,
and an explicit equivalent, overlap, distractor, or conflict annotation. The
catalog must be reviewed before experimental outcomes are observed. Do not
revise it after a freeze; create a new protocol version instead.
