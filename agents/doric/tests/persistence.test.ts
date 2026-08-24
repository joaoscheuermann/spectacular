import assert from 'node:assert/strict';
import test from 'node:test';

import { createConfigStore } from '../src/lib/config-store.js';
import { createDatabase } from '../src/lib/database.js';
import { createSessionStore } from '../src/lib/sessions.js';

const connectionString = process.env.DORIC_TEST_DATABASE_URL;

test(
  'persists generic sessions messages contiguous events cascade and reconciliation',
  { skip: connectionString === undefined },
  async () => {
    assert.ok(connectionString);
    const database = createDatabase(connectionString);
    const configs = createConfigStore(database);
    const sessions = createSessionStore(database);

    try {
      const initial = await configs.load();
      const first = await sessions.create(initial);
      assert.deepEqual(first.messages, []);
      assert.equal('prompt' in first.session, false);
      assert.equal('result' in first.session, false);
      assert.equal(
        (await sessions.markReady(first.session.id))?.state,
        'ready',
      );

      const accepted = await sessions.acceptPrompt(first.session.id, promptId);
      assert.equal(accepted.status, 'accepted');
      await Promise.all([
        sessions.appendEvent(first.session.id, promptId, {
          type: 'reasoning.delta',
          delta: 'one',
        }),
        sessions.appendEvent(first.session.id, promptId, {
          type: 'response.started',
          model: 'test',
        }),
      ]);
      assert.deepEqual(
        (await sessions.eventsAfter(first.session.id, 0)).map(
          ({ sequence }) => sequence,
        ),
        [1, 2, 3],
      );

      assert.equal(
        (await sessions.markRunning(first.session.id))?.state,
        'running',
      );
      const messages = [
        { role: 'user' as const, content: 'first' },
        {
          role: 'assistant' as const,
          content: 'done',
          replay: [{ type: 'reasoning', encrypted_content: 'opaque' }],
        },
      ];
      assert.equal(
        (await sessions.finishPrompt(first.session.id, messages))?.state,
        'ready',
      );
      assert.deepEqual(
        (await sessions.find(first.session.id))?.messages,
        messages,
      );

      const replacementInput = structuredClone(initial.configuration);
      replacementInput.models.execution.model = 'replacement-executor';
      const replacement = await configs.replace(replacementInput);
      const second = await sessions.create(replacement);
      assert.equal(second.session.configRevision, replacement.revision);
      assert.equal(
        (await sessions.find(first.session.id))?.snapshot.revision,
        initial.revision,
      );

      assert.equal(
        (await sessions.requestCancellation(first.session.id))?.state,
        'cancelling',
      );
      assert.equal(
        (await sessions.finish(first.session.id, 'cancelled'))?.state,
        'cancelled',
      );
      assert.equal(await sessions.delete(first.session.id), 'deleted');
      assert.deepEqual(await sessions.eventsAfter(first.session.id, 0), []);

      assert.equal(await sessions.reconcile(), 1);
      const interrupted = await sessions.find(second.session.id);
      assert.equal(interrupted?.session.state, 'failed');
      assert.equal(interrupted?.session.errorCode, 'process_interrupted');
      assert.equal(await sessions.delete(second.session.id), 'deleted');

      const preserved = await configs.load();
      assert.equal(preserved.revision, replacement.revision);
      assert.equal(
        preserved.configuration.models.execution.model,
        'replacement-executor',
      );
    } finally {
      await database.$disconnect();
    }
  },
);

const promptId = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1602';
