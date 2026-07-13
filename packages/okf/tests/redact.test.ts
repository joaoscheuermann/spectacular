import assert from 'node:assert/strict';
import { test } from 'node:test';

import { redactRegex } from '../src/lib/redact.js';

test('redacts regex literals in supported expression and keyword contexts', () => {
  const input = String.raw`const digit = /foo\d+/gi;
const escaped = /a\/b\\c/m;
const classed = /[a/b\]]+/u;
const grouped = (/ab+c/);
const record = { pattern: /x{2}/ };
const predicate = (value: string) => /ok/.test(value);
return /returned/g;
case /case/i:
throw /thrown/;
yield /yielded/y;
  /line-start/s;`;

  assert.equal(
    redactRegex(input),
    `const digit = [regex redacted];
const escaped = [regex redacted];
const classed = [regex redacted];
const grouped = ([regex redacted]);
const record = { pattern: [regex redacted] };
const predicate = (value: string) => [regex redacted].test(value);
return [regex redacted];
case [regex redacted]:
throw [regex redacted];
yield [regex redacted];
  [regex redacted];`,
  );
});

test('redacts multiple regex literals independently', () => {
  assert.equal(
    redactRegex('const first = /one/; const second = /two/g;'),
    'const first = [regex redacted]; const second = [regex redacted];',
  );
});

test('preserves comments URLs and ordinary division', () => {
  const input = String.raw`const quotient = total / count;
const nested = (total / count);
const url = "https://example.com/a/b";
const secure = 'https://example.com/path';
// const commented = /hidden/g;
// return /also-hidden/;
/* const blocked = /hidden/i; */
/* throw /also-hidden/; */
const commentText = "// /not-code/";`;

  assert.equal(redactRegex(input), input);
});

test('does not match across line boundaries', () => {
  const input = `const broken = /first
second/gi;`;

  assert.equal(redactRegex(input), input);
});

test('preserves division after member and dollar-prefixed keyword lookalikes', () => {
  const input = `obj.return / first / second;
$return / first / second;`;

  assert.equal(redactRegex(input), input);
});

test('redacts only valid unique ECMAScript flag combinations', () => {
  const input = `const legacy = /x/dgimsuy;
const sets = /x/v;
const unknown = /x/z;
const suffix = /x/gz;
const duplicate = /x/gg;
const modes = /x/uv;`;

  assert.equal(
    redactRegex(input),
    `const legacy = [regex redacted];
const sets = [regex redacted];
const unknown = /x/z;
const suffix = /x/gz;
const duplicate = /x/gg;
const modes = /x/uv;`,
  );
});

test('redacts comment-shaped character classes and later same-line literals', () => {
  const input = `const slashes = /[//]+/g; const next = /next/;
const blocks = /[/*]+/; const after = /after/i;`;

  assert.equal(
    redactRegex(input),
    `const slashes = [regex redacted]; const next = [regex redacted];
const blocks = [regex redacted]; const after = [regex redacted];`,
  );
});

test('preserves shaped comments and redacts literals after closed blocks', () => {
  const input = `// return /line/;
/*
const hidden = /block/g;
throw /keyword/;
*/
const visible = /shown/g;
/* const inline = /hidden/; */ const after = /after/;`;

  assert.equal(
    redactRegex(input),
    `// return /line/;
/*
const hidden = /block/g;
throw /keyword/;
*/
const visible = [regex redacted];
/* const inline = /hidden/; */ const after = [regex redacted];`,
  );
});

test('preserves simple strings and URLs before later same-line literals', () => {
  const input = String.raw`const url = "https://example.com/a/b"; const first = /one/;
const text = 'return /not-code/'; const second = /two/g;
const comment = "/* const hidden = /nope/; */"; const third = /three/;`;

  assert.equal(
    redactRegex(input),
    `const url = "https://example.com/a/b"; const first = [regex redacted];
const text = 'return /not-code/'; const second = [regex redacted];
const comment = "/* const hidden = /nope/; */"; const third = [regex redacted];`,
  );
});

test('uses line-start context independently after CRLF', () => {
  assert.equal(
    redactRegex('const previous =\r\n  /next/u;'),
    'const previous =\r\n  [regex redacted];',
  );
});
