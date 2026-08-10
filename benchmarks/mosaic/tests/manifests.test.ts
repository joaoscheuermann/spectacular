import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import test from 'node:test';
import { parse } from '@iarna/toml';

type Manifest = {
  readonly contract_version: string;
  readonly name: string;
  readonly protocol: string;
  readonly api_protocol: string;
  readonly install_cmd: string;
  readonly launch_cmd: string;
  readonly supports_acp_set_model: boolean;
  readonly default_model: string;
  readonly skill_paths: readonly string[];
  readonly home_dirs: readonly string[];
  readonly env_mapping: Readonly<Record<string, string>>;
};

const paths = ['mosaic-direct', 'mosaic'] as const;
const projectRoot = resolve(process.cwd(), 'benchmarks/mosaic');

const load = async (name: (typeof paths)[number]): Promise<Manifest> =>
  parse(
    await readFile(
      resolve(projectRoot, 'agents', name, 'manifest.toml'),
      'utf8',
    ),
  ) as unknown as Manifest;

const mapping = {
  BENCHFLOW_PROVIDER_BASE_URL: 'OPENROUTER_BASE_URL',
  BENCHFLOW_PROVIDER_API_KEY: 'OPENROUTER_API_KEY',
  BENCHFLOW_PROVIDER_MODEL: 'OPENROUTER_MODEL',
};

const assertContract = (manifest: Manifest): void => {
  assert.equal(manifest.contract_version, '1.0');
  assert.equal(manifest.protocol, 'acp');
  assert.equal(manifest.api_protocol, 'openai-completions');
  assert.equal(manifest.supports_acp_set_model, false);
  assert.equal(manifest.default_model, 'openrouter/openai/gpt-5.6-luna');
  assert.deepEqual(manifest.skill_paths, ['$HOME/.agents/skills']);
  assert.deepEqual(manifest.home_dirs, ['.agents']);
  assert.deepEqual(manifest.env_mapping, mapping);
  assert.match(manifest.install_cmd, /BF_NODE_VERSION=22\.20\.0/);
  assert.match(manifest.install_cmd, /x86_64\|amd64\) node_arch=x64/);
  assert.match(manifest.install_cmd, /aarch64\|arm64\) node_arch=arm64/);
  assert.match(manifest.install_cmd, /mosaic-benchmark-v0\.1\.2/);
  assert.match(manifest.install_cmd, /BF_BUNDLE_SHA256=[a-f0-9]{64}/);
  assert.match(
    manifest.install_cmd,
    /node_sha256=00bbd05e306ea68b6e13e17360d0e2f680b493ef95f2fea1c4296ff7437530bc/,
  );
  assert.match(
    manifest.install_cmd,
    /node_sha256=06907b9c088ce62305bc1530e5c1ae1510245114645768f7750c349c5b6fe667/,
  );
  assert.match(manifest.install_cmd, /downloaded_checksum/);
  assert.match(
    manifest.install_cmd,
    /sha256sum -c mosaic-bench-acp\.mjs\.sha256/,
  );
  assert.match(
    manifest.install_cmd,
    /curl -fsSL -o "\$tmp\/mosaic-bench-acp\.mjs"/,
  );
  assert.match(
    manifest.install_cmd,
    /curl -fsSL -o "\$tmp\/mosaic-bench-acp\.mjs\.sha256"/,
  );
  assert.doesNotMatch(manifest.install_cmd, /\b(latest|main|master)\b/i);
  assert.match(manifest.launch_cmd, /umask 077/);
  assert.match(manifest.launch_cmd, /key_file=\$\(mktemp\)/);
  assert.match(manifest.launch_cmd, /chmod 600 "\$key_file"/);
  assert.match(manifest.launch_cmd, /trap cleanup 0 HUP INT TERM/);
  assert.match(
    manifest.launch_cmd,
    /unset OPENROUTER_API_KEY BENCHFLOW_PROVIDER_API_KEY BENCHFLOW_LITELLM_MASTER_KEY/,
  );
  assert.match(
    manifest.launch_cmd,
    /export OPENROUTER_API_KEY_FILE="\$key_file"/,
  );
};

test('parses BenchFlow v1 Mosaic agent manifests', async () => {
  const manifests = await Promise.all(paths.map(load));
  const bundleChecksum = createHash('sha256')
    .update(
      await readFile(resolve(projectRoot, 'dist', 'mosaic-bench-acp.mjs')),
    )
    .digest('hex');

  manifests.forEach(assertContract);
  manifests.forEach((manifest) =>
    assert.match(
      manifest.install_cmd,
      new RegExp(`BF_BUNDLE_SHA256=${bundleChecksum}`),
    ),
  );
  assert.deepEqual(
    manifests.map((manifest) => manifest.name),
    ['mosaic-direct', 'mosaic'],
  );
  assert.equal(
    manifests[0]?.launch_cmd.endsWith(
      'exec /opt/benchflow/node/bin/node /opt/benchflow/mosaic/mosaic-bench-acp.mjs serve direct',
    ),
    true,
  );
  assert.equal(
    manifests[1]?.launch_cmd.endsWith(
      'exec /opt/benchflow/node/bin/node /opt/benchflow/mosaic/mosaic-bench-acp.mjs serve mosaic',
    ),
    true,
  );
});

test('keeps both manifests equivalent outside their declared execution arm', async () => {
  const [direct, mosaic] = await Promise.all(paths.map(load));
  const {
    name: _directName,
    launch_cmd: _directLaunch,
    ...directCommon
  } = direct!;
  const {
    name: _mosaicName,
    launch_cmd: _mosaicLaunch,
    ...mosaicCommon
  } = mosaic!;

  assert.deepEqual(directCommon, mosaicCommon);
});
