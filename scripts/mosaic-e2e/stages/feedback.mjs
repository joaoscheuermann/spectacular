/*
Render the latest completion judgment and accumulated judge observations as feedback.
Receives the evaluation and observations; returns the next executor user message so
the existing conversation can continue without replacing its fixed acceptance criteria.
**/

import * as prompt from '../context.mjs';

export const feedbackInput = ({ evaluation, observations }) =>
  [
    prompt.section('Completion feedback', evaluation),
    prompt.section('Judge observations', observations),
  ].join('\n\n');
