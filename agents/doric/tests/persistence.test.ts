import assert from 'node:assert/strict';
import test from 'node:test';

import { createConfigStore } from '../src/lib/config-store.js';
import { createDatabase } from '../src/lib/database.js';
import { createSessionStore } from '../src/lib/sessions.js';

const connectionString = process.env.DORIC_TEST_DATABASE_URL;

test(
  'persists revisions snapshots pagination cancellation replay and restart reconciliation',
  { skip: connectionString === undefined },
  async () => {
    assert.ok(connectionString);
    const database = createDatabase(connectionString);
    const configs = createConfigStore(database);
    const sessions = createSessionStore(database);

    try {
      const initial = await configs.load();
      const changed = structuredClone(initial.configuration);
      changed.models.planning.model = 'replacement-planner';

      const first = await sessions.create('first', initial);
      const replacement = await configs.replace(changed);
      assert.equal(replacement.revision, initial.revision + 1);
      assert.equal(
        (await sessions.find(first.session.id))?.snapshot.configuration.models
          .planning.model,
        initial.configuration.models.planning.model,
      );

      const second = await sessions.create('second', replacement);
      const page = await sessions.list(1);
      assert.equal(page.sessions.length, 1);
      assert.equal(page.nextCursor, page.sessions[0]?.id);
      const next = await sessions.list(1, page.nextCursor);
      assert.deepEqual(
        new Set([page.sessions[0]?.id, next.sessions[0]?.id]),
        new Set([first.session.id, second.session.id]),
      );

      await sessions.appendEvent(
        second.session.id,
        event(second.session.id, 1),
      );
      await sessions.appendEvent(
        second.session.id,
        event(second.session.id, 2),
      );
      assert.deepEqual(
        (await sessions.eventsAfter(second.session.id, 0)).map(
          ({ sequence }) => sequence,
        ),
        [1, 2],
      );
      await assert.rejects(
        sessions.appendEvent(second.session.id, event(second.session.id, 4)),
        /sequence/u,
      );

      assert.equal(
        (await sessions.requestCancellation(first.session.id))?.state,
        'cancelling',
      );
      assert.equal(
        (
          await sessions.finish(first.session.id, 'completed', {
            ignored: true,
          })
        )?.state,
        'cancelled',
      );
      assert.equal(await sessions.delete(first.session.id), 'deleted');

      assert.equal(
        (await sessions.markRunning(second.session.id))?.state,
        'running',
      );
      assert.equal(await sessions.delete(second.session.id), 'active');
      assert.equal(await sessions.reconcile(), 1);
      const interrupted = await sessions.find(second.session.id);
      assert.equal(interrupted?.session.state, 'failed');
      assert.equal(interrupted?.session.errorCode, 'process_interrupted');
      assert.deepEqual(
        (await sessions.eventsAfter(second.session.id, 0)).map(
          ({ sequence }) => sequence,
        ),
        [1, 2],
      );
      assert.equal(await sessions.delete(second.session.id), 'deleted');

      const serialized = JSON.stringify(await configs.load());
      assert.doesNotMatch(serialized, /private-token/u);
      assert.match(serialized, /OPENROUTER_API_KEY/u);
    } finally {
      await database.$disconnect();
    }
  },
);

const event = (runId: string, sequence: number) => ({
  schemaVersion: 3 as const,
  runId,
  sequence,
  type: 'stage.started' as const,
  stage: 'plan' as const,
});
