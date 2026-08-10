import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { compare } from '../src/compare.js';

type Json = Record<string, unknown>;

const hash = (value: string): string =>
  createHash('sha256').update(value).digest('hex');
const taskDigest = `sha256:${'a'.repeat(64)}`;
const taskSource = {
  type: 'github',
  repo: 'benchflow-ai/skillsbench',
  requested_ref: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af',
  resolved_sha: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af',
  path: 'tasks/edit-pdf',
  dirty: false,
  file_hashes: { 'task.md': `sha256:${'b'.repeat(64)}` },
};

const writeArm = async (
  root: string,
  agent: 'mosaic-direct' | 'mosaic',
  reward: number,
  cost: number,
): Promise<string> => {
  const directory = join(root, agent);
  await mkdir(join(directory, 'jobs', 'one'), { recursive: true });
  const taskManifest = JSON.stringify({
    schema_version: 1,
    total: 1,
    source: taskSource,
    tasks: [
      {
        task_id: 'edit-pdf',
        digest: taskDigest,
        registry_digest_match: true,
      },
    ],
  });
  const runConfig = JSON.stringify({
    schema_version: 1,
    eval: {
      agent,
      model: 'openrouter/openai/gpt-5.6-luna',
      reasoning_effort: null,
      environment: 'docker',
      concurrency: 1,
      build_concurrency: 1,
      skill_mode: 'with-skill',
      usage_tracking: { requested: 'required' },
      agent_env_keys: [],
      skills_dir: null,
      include_tasks: [],
      exclude_tasks: [],
      source_provenance: taskSource,
      dataset_name: null,
      dataset_version: null,
    },
    retry_attempts: 0,
  });
  const health = JSON.stringify({
    schema_version: 1,
    total_rows: 1,
    scored_rows: 1,
    unscored_rows: 0,
    missing_llm_trajectory: 0,
    malformed_llm_trajectory: 0,
    rows_with_tool_calls: 0,
    zero_tool_rows: 1,
    rows: [
      {
        task_id: 'edit-pdf',
        scored: true,
        error: null,
        verifier_error: null,
        reward,
        tool_calls: 0,
        has_llm_trajectory: true,
        valid_llm_trajectory: true,
        llm_trajectory_rows: 1,
      },
    ],
  });
  const agentManifest = `name = "${agent}"\n`;
  const bundle = 'common generated bundle\n';
  const metadata = {
    action: 'smoke',
    benchmark: 'skillsbench',
    benchflowVersion: '0.6.5',
    campaignId: 'skillsbench-smoke-fixture',
    source: {
      repo: 'benchflow-ai/skillsbench',
      path: 'tasks/edit-pdf',
      ref: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af',
    },
    expectedTasks: 1,
    agent,
    model: 'openrouter/openai/gpt-5.6-luna',
    effort: 'low',
    sandbox: 'docker',
    concurrency: 1,
    buildConcurrency: 1,
    retries: 0,
    loopStrategy: 'single-shot',
    usageTracking: 'required',
    skillMode: 'with-skill',
    digests: {
      taskManifest: hash(taskManifest),
      runConfig: hash(runConfig),
      health: hash(health),
      bundle: hash(bundle),
      agentManifest: hash(agentManifest),
    },
  };
  const result = {
    task_name: 'edit-pdf',
    agent,
    agent_name: 'mosaic-benchmark',
    model: 'openrouter/openai/gpt-5.6-luna',
    skill_mode: 'with-skill',
    loop: { strategy: 'single-shot' },
    usage_tracking: {
      requested: 'required',
      status: 'enabled',
      usage_source: 'provider_response',
    },
    trajectory_source: 'acp',
    partial_trajectory: false,
    n_prompts: 1,
    task_digest: taskDigest,
    source: taskSource,
    rewards: { reward },
    n_tool_calls: 0,
    error: null,
    verifier_error: null,
    agent_result: {
      usage_source: 'provider_response',
      price_source: 'litellm',
      n_tool_calls: 0,
      total_tokens: 100,
      cost_usd: cost,
    },
  };
  await Promise.all([
    writeFile(join(directory, 'metadata.json'), JSON.stringify(metadata)),
    writeFile(join(directory, 'task-manifest.json'), taskManifest),
    writeFile(join(directory, 'run-config.json'), runConfig),
    writeFile(join(directory, 'health.json'), health),
    writeFile(join(directory, 'agent-manifest.toml'), agentManifest),
    writeFile(join(directory, 'bundle.mjs'), bundle),
    writeFile(
      join(directory, 'jobs', 'one', 'result.json'),
      JSON.stringify(result),
    ),
  ]);
  return directory;
};

const updateMetadata = async (
  directory: string,
  mutate: (metadata: Json) => void,
): Promise<void> => {
  const path = join(directory, 'metadata.json');
  const metadata: Json = JSON.parse(await readFile(path, 'utf8'));
  mutate(metadata);
  await writeFile(path, JSON.stringify(metadata));
};

const updateArtifact = async (
  directory: string,
  file: string,
  digestKey: string,
  mutate: (artifact: Json) => void,
): Promise<void> => {
  const path = join(directory, file);
  const artifact: Json = JSON.parse(await readFile(path, 'utf8'));
  mutate(artifact);
  const text = JSON.stringify(artifact);
  await writeFile(path, text);
  await updateMetadata(directory, (metadata) => {
    (metadata.digests as Json)[digestKey] = hash(text);
  });
};

test('reports a valid SkillsBench Pareto win with benchmark proof', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 2),
    writeArm(root, 'mosaic', 0.8, 1),
  ]);
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.deepEqual(report.reasons, []);
  assert.deepEqual(
    {
      benchmark: report.benchmark,
      valid: report.valid,
      paretoWin: report.paretoWin,
      exitCode: report.exitCode,
    },
    { benchmark: 'skillsbench', valid: true, paretoWin: true, exitCode: 0 },
  );
});

test('reports a valid comparison without a Pareto win', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 1),
    writeArm(root, 'mosaic', 0.8, 2),
  ]);
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.deepEqual(
    {
      valid: report.valid,
      paretoWin: report.paretoWin,
      exitCode: report.exitCode,
    },
    { valid: true, paretoWin: false, exitCode: 1 },
  );
});

test('rejects arms from different campaign IDs or actions', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 2),
    writeArm(root, 'mosaic', 0.8, 1),
  ]);
  await updateMetadata(mosaic, (metadata) => {
    metadata.campaignId = 'a-different-campaign';
  });
  let report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /metadata differs: campaignId/);
  await updateMetadata(mosaic, (metadata) => {
    metadata.campaignId = 'skillsbench-smoke-fixture';
    metadata.action = 'run';
  });
  report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /metadata differs: action/);
  assert.match(report.reasons.join('\n'), /pinned benchmark contract/);
});

test('rejects invalid task-manifest and run-config source provenance', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 2),
    writeArm(root, 'mosaic', 0.8, 1),
  ]);
  await updateArtifact(
    mosaic,
    'task-manifest.json',
    'taskManifest',
    (artifact) => {
      delete (artifact.source as Json).file_hashes;
      (artifact.tasks as Json[])[0]!.registry_digest_match = false;
    },
  );
  await updateArtifact(mosaic, 'run-config.json', 'runConfig', (artifact) => {
    ((artifact.eval as Json).source_provenance as Json).resolved_sha = 'wrong';
  });
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /mosaic: invalid task manifest/);
  assert.match(report.reasons.join('\n'), /mosaic: invalid run config/);
});

test('rejects non-agent run-config differences between arms', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 2),
    writeArm(root, 'mosaic', 0.8, 1),
  ]);
  await updateArtifact(mosaic, 'run-config.json', 'runConfig', (artifact) => {
    artifact.retry_policy = 'different';
  });
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /run configs differ/);
});

test('rejects BenchFlow-owned ACP reasoning effort', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 2),
    writeArm(root, 'mosaic', 0.8, 1),
  ]);
  await updateArtifact(mosaic, 'run-config.json', 'runConfig', (artifact) => {
    (artifact.eval as Json).reasoning_effort = 'low';
  });
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /mosaic: invalid run config/);
});

test('rejects health evidence that diverges from public results', async (t) => {
  const mutations: readonly [string, (health: Json) => void][] = [
    [
      'reward equality',
      (health) => {
        (health.rows as Json[])[0]!.reward = 0.7;
      },
    ],
    [
      'reward range',
      (health) => {
        (health.rows as Json[])[0]!.reward = 1.1;
      },
    ],
    [
      'tool call equality',
      (health) => {
        (health.rows as Json[])[0]!.tool_calls = 1;
      },
    ],
    [
      'tool call range',
      (health) => {
        (health.rows as Json[])[0]!.tool_calls = -1;
      },
    ],
    [
      'has trajectory',
      (health) => {
        (health.rows as Json[])[0]!.has_llm_trajectory = false;
      },
    ],
    [
      'valid trajectory',
      (health) => {
        (health.rows as Json[])[0]!.valid_llm_trajectory = false;
      },
    ],
    [
      'trajectory rows',
      (health) => {
        (health.rows as Json[])[0]!.llm_trajectory_rows = 0;
      },
    ],
    [
      'rows with calls',
      (health) => {
        health.rows_with_tool_calls = 1;
      },
    ],
    [
      'zero-tool rows',
      (health) => {
        health.zero_tool_rows = 0;
      },
    ],
  ];
  for (const [name, mutate] of mutations) {
    const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    const [direct, mosaic] = await Promise.all([
      writeArm(root, 'mosaic-direct', 0.5, 2),
      writeArm(root, 'mosaic', 0.8, 1),
    ]);
    await updateArtifact(mosaic, 'health.json', 'health', mutate);
    const report = await compare({ directDir: direct, mosaicDir: mosaic });
    assert.equal(report.exitCode, 2, name);
    assert.match(
      report.reasons.join('\n'),
      /mosaic: invalid health summary/,
      name,
    );
  }
});

test('rejects unverifiable telemetry even when tool activity is zero', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 1),
    writeArm(root, 'mosaic', 0.8, 0),
  ]);
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /cost_usd/);
});

test('rejects an artifact changed after its metadata digest', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 1),
    writeArm(root, 'mosaic', 0.8, 0.5),
  ]);
  await writeFile(join(mosaic, 'run-config.json'), '{}');
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /runConfig digest mismatch/);
  assert.match(report.reasons.join('\n'), /invalid run config/);
  void direct;
});

test('rejects a source outside the fixed benchmark contract', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 1),
    writeArm(root, 'mosaic', 0.8, 0.5),
  ]);
  await updateMetadata(mosaic, (metadata) => {
    (metadata.source as Json).ref = 'unpinned';
  });
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /pinned benchmark contract/);
});

test('rejects task names that diverge from the manifest despite coherent health', async (t) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const [direct, mosaic] = await Promise.all([
    writeArm(root, 'mosaic-direct', 0.5, 1),
    writeArm(root, 'mosaic', 0.8, 0.5),
  ]);
  const health = JSON.stringify({
    schema_version: 1,
    total_rows: 1,
    scored_rows: 1,
    unscored_rows: 0,
    missing_llm_trajectory: 0,
    malformed_llm_trajectory: 0,
    rows_with_tool_calls: 0,
    zero_tool_rows: 1,
    rows: [
      {
        task_id: 'other-task',
        scored: true,
        error: null,
        verifier_error: null,
        reward: 0.8,
        tool_calls: 0,
        has_llm_trajectory: true,
        valid_llm_trajectory: true,
        llm_trajectory_rows: 1,
      },
    ],
  });
  await Promise.all([
    writeFile(join(mosaic, 'health.json'), health),
    writeFile(
      join(mosaic, 'jobs', 'one', 'result.json'),
      JSON.stringify({
        task_name: 'other-task',
        agent: 'mosaic',
        agent_name: 'mosaic-benchmark',
        model: 'openrouter/openai/gpt-5.6-luna',
        skill_mode: 'with-skill',
        loop: { strategy: 'single-shot' },
        usage_tracking: {
          requested: 'required',
          status: 'enabled',
          usage_source: 'provider_response',
        },
        trajectory_source: 'acp',
        partial_trajectory: false,
        n_prompts: 1,
        task_digest: taskDigest,
        source: taskSource,
        rewards: { reward: 0.8 },
        n_tool_calls: 0,
        error: null,
        verifier_error: null,
        agent_result: {
          usage_source: 'provider_response',
          price_source: 'litellm',
          n_tool_calls: 0,
          total_tokens: 100,
          cost_usd: 0.5,
        },
      }),
    ),
  ]);
  await updateMetadata(mosaic, (metadata) => {
    (metadata.digests as Json).health = hash(health);
  });
  const report = await compare({ directDir: direct, mosaicDir: mosaic });
  assert.equal(report.exitCode, 2);
  assert.match(report.reasons.join('\n'), /mosaic: invalid task manifest/);
});

test('rejects every mutated BenchFlow result identity field', async (t) => {
  const mutations: readonly [string, (result: Json) => void][] = [
    [
      'agent',
      (result) => {
        result.agent = 'other';
      },
    ],
    [
      'agent_name',
      (result) => {
        result.agent_name = 'other';
      },
    ],
    [
      'model',
      (result) => {
        result.model = 'other';
      },
    ],
    [
      'skill_mode',
      (result) => {
        result.skill_mode = 'no-skill';
      },
    ],
    [
      'loop.strategy',
      (result) => {
        (result.loop as Json).strategy = 'other';
      },
    ],
    [
      'usage_tracking',
      (result) => {
        (result.usage_tracking as Json).usage_source = 'untrusted';
      },
    ],
    [
      'usage_tracking',
      (result) => {
        (result.usage_tracking as Json).status = 'disabled';
      },
    ],
    [
      'trajectory_source',
      (result) => {
        result.trajectory_source = 'other';
      },
    ],
    [
      'partial_trajectory',
      (result) => {
        result.partial_trajectory = true;
      },
    ],
    [
      'n_prompts',
      (result) => {
        result.n_prompts = 2;
      },
    ],
    [
      'agent_result.n_tool_calls',
      (result) => {
        (result.agent_result as Json).n_tool_calls = 1;
      },
    ],
    [
      'task_digest',
      (result) => {
        result.task_digest = `sha256:${'c'.repeat(64)}`;
      },
    ],
    [
      'source',
      (result) => {
        (result.source as Json).resolved_sha = 'wrong';
      },
    ],
    [
      'price_source',
      (result) => {
        delete (result.agent_result as Json).price_source;
      },
    ],
    [
      'price_source',
      (result) => {
        (result.agent_result as Json).price_source = 'unknown';
      },
    ],
    [
      'reward',
      (result) => {
        (result.rewards as Json).reward = -1;
      },
    ],
    [
      'reward',
      (result) => {
        (result.rewards as Json).reward = 1.1;
      },
    ],
  ];
  for (const [field, mutate] of mutations) {
    const root = await mkdtemp(join(tmpdir(), 'mosaic-compare-'));
    t.after(() => rm(root, { recursive: true, force: true }));
    const [direct, mosaic] = await Promise.all([
      writeArm(root, 'mosaic-direct', 0.5, 2),
      writeArm(root, 'mosaic', 0.8, 1),
    ]);
    const path = join(mosaic, 'jobs', 'one', 'result.json');
    const result: Json = JSON.parse(await readFile(path, 'utf8'));
    mutate(result);
    await writeFile(path, JSON.stringify(result));
    const report = await compare({ directDir: direct, mosaicDir: mosaic });
    assert.equal(report.exitCode, 2, field);
    assert.match(
      report.reasons.join('\n'),
      new RegExp(`mosaic:0: invalid ${field.replace('.', '\\.')}`),
      field,
    );
  }
});
