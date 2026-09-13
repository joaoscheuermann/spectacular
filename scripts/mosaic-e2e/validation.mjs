export const hasDuplicates = (values) => new Set(values).size !== values.length;

export const hasExactMembers = (actual, expected) => {
  if (actual.length !== expected.length || hasDuplicates(actual)) {
    return false;
  }

  return actual.every((value) => expected.includes(value));
};

/** Preserve input order among ready nodes; reject cycles and unknown prerequisites. */
export const ordered = (nodes) => {
  if (hasDuplicates(nodes.map(({ id }) => id))) {
    throw new Error('Duplicate node IDs.');
  }

  const remaining = [...nodes];
  const completedIds = new Set();
  const sorted = [];

  while (remaining.length > 0) {
    const readyIndex = remaining.findIndex(({ dependencies }) =>
      dependencies.every((id) => completedIds.has(id)),
    );

    if (readyIndex === -1) {
      throw new Error('Cycle or unknown dependency.');
    }

    const [ready] = remaining.splice(readyIndex, 1);

    sorted.push(ready);

    completedIds.add(ready.id);
  }

  return sorted;
};
