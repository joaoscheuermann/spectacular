export const unique = <Value extends { readonly name: string }>(
  values: readonly Value[],
): Value[] => {
  const names = new Set<string>();
  return values.filter(
    ({ name }) => !names.has(name) && Boolean(names.add(name)),
  );
};
