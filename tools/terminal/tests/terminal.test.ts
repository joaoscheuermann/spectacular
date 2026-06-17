import { access, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { createTool } from '../src/index.js';

describe('terminal tool', () => {
  test('executes a command from the requested working directory', async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), 'doric-terminal-'));
    const command = os.platform() === 'win32' ? "'hello'" : "printf 'hello'";

    const result = await createTool({ workspaceRoot: root }).execute({
      command,
      timeout_ms: 5000,
    });

    assert.equal(result.schema, 'terminal.compact.v1');
    assert.equal(result.exit_code, 0);
    assert.equal(result.success, true);
    assert.match(result.stdout.head.join('\n'), /hello/);
    await rm(root, { recursive: true, force: true });
  });

  test('returns timeout diagnostics with exit code -1', async () => {
    const root = await mkdtemp(
      path.join(os.tmpdir(), 'doric-terminal-timeout-'),
    );
    const command =
      os.platform() === 'win32'
        ? 'Start-Sleep -Milliseconds 300'
        : 'node -e "setTimeout(() => {}, 300)"';

    const result = await createTool({ workspaceRoot: root }).execute({
      command,
      timeout_ms: 10,
    });

    assert.equal(result.exit_code, -1);
    assert.equal(result.success, false);
    assert.match(result.stderr.head.join('\n'), /timed out/);
    await rm(root, { recursive: true, force: true });
  });

  test('terminates descendant processes when a command times out', async () => {
    const root = await mkdtemp(
      path.join(os.tmpdir(), 'doric-terminal-tree-timeout-'),
    );
    const marker = path.join(root, 'marker.txt');
    const child = path.join(root, 'child.js');
    const parent = path.join(root, 'parent.js');
    await writeFile(
      child,
      'setTimeout(() => require("node:fs").writeFileSync(process.argv[2], "alive"), 400);',
      'utf8',
    );
    await writeFile(
      parent,
      `const { spawn } = require("node:child_process");
spawn(process.execPath, [${JSON.stringify(child)}, ${JSON.stringify(marker)}], { stdio: "ignore" });
setTimeout(() => {}, 2000);
`,
      'utf8',
    );

    const result = await createTool({ workspaceRoot: root }).execute({
      command: `node ${quote(parent)}`,
      timeout_ms: 50,
    });
    await wait(800);

    assert.equal(result.exit_code, -1);
    await assert.rejects(access(marker));
    await rm(root, { recursive: true, force: true });
  });

  test('extracts diagnostics from stderr', async () => {
    const root = await mkdtemp(
      path.join(os.tmpdir(), 'doric-terminal-diagnostics-'),
    );
    const script = path.join(root, 'diagnostic.js');
    await writeFile(
      script,
      'console.error("src/index.ts(1,2): error TS2304: Cannot find name x"); process.exit(1);',
      'utf8',
    );

    const result = await createTool({ workspaceRoot: root }).execute({
      command: `node ${quote(script)}`,
      timeout_ms: 5000,
    });

    assert.equal(result.success, false);
    assert.equal(result.diagnostics[0]?.kind, 'typescript_error');
    assert.equal(result.diagnostics[0]?.stream, 'stderr');
    assert.match(result.diagnostics[0]?.text ?? '', /TS2304/);
    await rm(root, { recursive: true, force: true });
  });
});

const quote = (value: string): string =>
  os.platform() === 'win32'
    ? `"${value.replaceAll('"', '\\"')}"`
    : `'${value.replaceAll("'", "'\\''")}'`;

const wait = (ms: number): Promise<void> =>
  new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
