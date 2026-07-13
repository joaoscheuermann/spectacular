export const optimizerSystemPrompt = [
  'Improve one system prompt against the supplied training assertions.',
  'Return a complete replacement prompt, not a patch.',
  'Use only the training scenarios, current failures, and matching history supplied.',
  'Do not return a prompt equivalent to the incumbent or any previously attempted prompt.',
  'Do not reuse a previously failed strategy.',
  'Preserve the original task intent while correcting observed failures.',
  'Return only JSON with prompt:string and strategy:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

export const compressionSystemPrompt = [
  'Compress the supplied system prompt without changing its behavior.',
  'Return a complete prompt that is 20 to 30 percent shorter by character count.',
  'Use only the supplied training contract and never weaken an assertion.',
  'Return only JSON with prompt:string and strategy:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

export const resultJudgeSystemPrompt = [
  'Apply every binary assertion independently to every supplied model output.',
  'Return exactly one verdict for each eval id and sample index pair.',
  'Sample indices are zero-based integers 0, 1, and 2.',
  'Set passed only when the output clearly satisfies the assertion.',
  'Give concise evidence-based reasoning.',
  'Return only JSON with results:array of {evalId:string,sampleIndex:number,reasoning:string,passed:boolean}.',
  'Do not wrap the JSON in Markdown.',
].join(' ');
