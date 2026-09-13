export const hasExactMembers = (actual, expected) =>
  actual.length === expected.length &&
  new Set(actual).size === actual.length &&
  actual.every((value) => expected.includes(value));
