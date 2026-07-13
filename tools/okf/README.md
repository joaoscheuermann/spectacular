# tool-okf

Provider-neutral, read-only search over Open Knowledge Format bundles stored at
`<workspace>/.agents/bundles`.

The exported `createTool({ workspaceRoot, sandbox })` factory creates the
`okf_search` tool. Its input accepts a natural-language `query`, an optional
top-level `bundle`, and an optional result `limit`. Results contain OKF concept
metadata and bounded Markdown content ordered by deterministic lexical
relevance.

The tool never writes files, accesses the network, follows symlinks, or reads
outside the fixed bundle root. Reserved `index.md` and `log.md` files are not
treated as concepts.

## Building

Run `npx nx build tool-okf` to build the library.

## Testing

Run `npx nx run tool-okf:test` to test the library.
