import { randomInt } from 'node:crypto';

export const armLabels = {
  direct: 'Direct',
  directGoal: 'Direct Goal',
  requestP1: 'Request P1',
  goalP1: 'Goal P1',
};

export const pairs = [
  {
    id: 'direct-vs-request-p1',
    left: 'direct',
    right: 'requestP1',
  },
  {
    id: 'direct-vs-direct-goal',
    left: 'direct',
    right: 'directGoal',
  },
  {
    id: 'request-p1-vs-goal-p1',
    left: 'requestP1',
    right: 'goalP1',
  },
  {
    id: 'direct-goal-vs-goal-p1',
    left: 'directGoal',
    right: 'goalP1',
  },
  {
    id: 'direct-vs-goal-p1',
    left: 'direct',
    right: 'goalP1',
  },
];

export const pairLabel = ({ left, right }) =>
  `${armLabels[left]} × ${armLabels[right]}`;

export const orientations = (plans, pair) => {
  const leftOption = randomInt(2) === 0 ? 'a' : 'b';
  const first = {
    leftOption,
    optionA: leftOption === 'a' ? plans[pair.left] : plans[pair.right],
    optionB: leftOption === 'b' ? plans[pair.left] : plans[pair.right],
  };

  return [
    first,
    {
      leftOption: leftOption === 'a' ? 'b' : 'a',
      optionA: first.optionB,
      optionB: first.optionA,
    },
  ];
};

const outcome = (pair, leftOption, choice) => {
  if (choice === 'both' || choice === 'neither') return choice;
  return choice === leftOption ? pair.left : pair.right;
};

export const summarizeJudgment = (pair, options, response, index) => ({
  orientation: index + 1,
  leftOption: options[index].leftOption,
  choice: response.choice,
  rationale: response.rationale.trim(),
  winner: outcome(pair, options[index].leftOption, response.choice),
});

export const summarize = (pair, options, responses) => {
  const judgments = responses.map((response, index) =>
    summarizeJudgment(pair, options, response, index),
  );
  const winner = judgments.every(
    (judgment) => judgment.winner === judgments[0].winner,
  )
    ? judgments[0].winner
    : 'inconsistent';

  return { ...pair, judgments, winner };
};
