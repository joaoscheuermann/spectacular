export const optimizerSystemPrompt = [
  'Evolve one system prompt for one target model.',
  'Return a complete replacement prompt, not a patch.',
  'Optimize only the target described in the input and use only its history.',
  'Preserve the original intent while correcting the listed failures.',
  'Independently screened scenario additions are optional.',
  'Each scenario input must contain only the model input.',
  'Each expected value must describe the observable successful behavior.',
  'Do not duplicate an incumbent or another proposed scenario.',
  'Return only JSON with prompt:string, scenarios:array of {id:string,input:string,expected:string,rationale:string|null,tags:string[]}, and rationale:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

export const scenarioInitializerSystemPrompt = [
  'Generate a shared baseline suite for testing one immutable original system prompt.',
  'Propose clear, useful scenarios that cover observable behavior required by the prompt.',
  'Do not replace or revise the original prompt and do not assume a target model.',
  'Use prior rejection feedback to improve later attempts.',
  'Do not duplicate any previously attempted candidate.',
  'Each scenario input must contain only the model input.',
  'Each expected value must describe the observable successful behavior.',
  'Return at least one scenario.',
  'Return only JSON with scenarios:non-empty array of {id:string,input:string,expected:string,rationale:string|null,tags:string[]} and rationale:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

export const resultJudgeSystemPrompt = [
  'Judge whether a model output satisfies the supplied expected behavior.',
  'Use only the supplied input, expected behavior, and output.',
  'Set passed only when the output clearly satisfies the expectation.',
  'Set ambiguous when the expectation or evidence permits multiple defensible judgments.',
  'Give a concise rationale.',
  'Return only JSON with passed:boolean, ambiguous:boolean, and rationale:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

export const scenarioJudgeSystemPrompt = [
  'Judge whether a proposed evaluation scenario is clear, useful, and internally consistent.',
  'Use the immutable original system prompt as the task contract.',
  'Set passed only when the expected behavior is an objective fit for that contract and input.',
  'Set ambiguous when multiple materially different expectations would be equally defensible.',
  'Do not rewrite the scenario. Give a concise rationale.',
  'Return only JSON with passed:boolean, ambiguous:boolean, and rationale:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');
