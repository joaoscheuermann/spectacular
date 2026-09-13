/** Render evidence as Markdown, including arbitrary source text and JSON. */
export const section = (title, value) => {
  const text =
    typeof value === 'string' ? value : JSON.stringify(value, null, 2);
  const backtickRuns = text.match(/`+/g) ?? [];

  const longestRun = backtickRuns.reduce(
    (longest, run) => Math.max(longest, run.length),
    0,
  );
  const fence = '`'.repeat(Math.max(3, longestRun + 1));

  return `# ${title}\n\n${fence}${typeof value === 'string' ? 'text' : 'json'}\n${text}\n${fence}`;
};

export const context = (request, skills = []) =>
  [
    section('Original request', request),
    '# Selected skill bodies',
    ...skills.map(({ name, body }) => section(name, body)),
  ].join('\n\n');
