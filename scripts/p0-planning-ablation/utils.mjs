export const messages = (system, user) => [
  { role: 'system', content: system },
  { role: 'user', content: user },
];

/** Waits for every concurrent operation before surfacing any failures. */
export const allFulfilled = async (promises, message) => {
  const settled = await Promise.allSettled(promises);
  const failures = settled
    .filter(({ status }) => status === 'rejected')
    .map(({ reason }) => reason);
  if (failures.length > 0) throw new AggregateError(failures, message);
  return settled.map(({ value }) => value);
};
