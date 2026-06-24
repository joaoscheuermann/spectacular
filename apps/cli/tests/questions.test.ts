import assert from 'node:assert/strict';
import { PassThrough } from 'node:stream';
import test from 'node:test';

import { createPromptKit, type PromptInput } from 'prompt-kit';

import { askOpenQuestions } from '../src/lib/questions.js';

test('asks open questions through prompt-kit selections', async () => {
  const input = new PassThrough() as PassThrough & PromptInput;
  const output: string[] = [];
  const prompt = createPromptKit({
    input,
    output: {
      isTTY: false,
      write(text) {
        output.push(text);
      },
    },
  });
  const answer = askOpenQuestions(
    [
      {
        id: 'question-1',
        question: 'Which branch?',
        options: [
          { title: 'main', value: 'main' },
          { title: 'develop', value: 'develop' },
        ],
      },
    ],
    prompt,
  );

  input.write('2\n');

  assert.equal(await answer, '- Which branch?: develop');
  assert.match(output.join(''), /\[1\/1\] Which branch\?/u);
  assert.match(output.join(''), /develop/u);
});
