export const describeRuntimeDependency = (
  name: string,
  version: string,
): string =>
  `${dependencyPurpose(name)} Declared as a runtime dependency at ${version}.`;

export const describeDevelopmentDependency = (
  name: string,
  version: string,
): string =>
  `${dependencyPurpose(name)} Declared as a development dependency at ${version}.`;

const dependencyPurpose = (name: string): string => {
  const exact = dependencyPurposes[name];

  if (exact !== undefined) {
    return exact;
  }

  if (name.startsWith('@nx/')) {
    return `Nx plugin ${name} for workspace build and generator tooling.`;
  }

  if (name.startsWith('@swc/')) {
    return `SWC compiler package ${name} for fast JavaScript and TypeScript transforms.`;
  }

  if (name.startsWith('@types/')) {
    return `TypeScript declarations for ${name.slice('@types/'.length)}.`;
  }

  return `Dependency ${name}.`;
};

const dependencyPurposes: Record<string, string> = {
  '@nx/eslint': 'Nx plugin for ESLint integration in the workspace.',
  '@nx/js': 'Nx plugin for JavaScript and TypeScript projects.',
  '@nx/node': 'Nx plugin for Node.js application and library projects.',
  '@nx/vite': 'Nx plugin for Vite build and test integration.',
  '@swc/core':
    'SWC compiler core for fast JavaScript and TypeScript transforms.',
  '@swc/helpers': 'SWC runtime helper library for generated JavaScript.',
  commander: 'CLI command parser for Node.js applications.',
  eslint: 'JavaScript and TypeScript linting tool.',
  glob: 'File globbing library for matching repository paths.',
  ignore: 'Gitignore-compatible pattern matching library.',
  nx: 'Nx workspace task runner and project graph tooling.',
  pino: 'Structured JSON logger for Node.js runtime output.',
  'pino-pretty': 'Pretty-printer for local Pino log output.',
  prettier: 'Opinionated code formatter.',
  tsx: 'TypeScript execution runtime for Node.js scripts.',
  tslib: 'TypeScript runtime helper library.',
  typescript: 'TypeScript compiler and language tooling.',
  vitest: 'Vite-native unit test runner.',
  zod: 'TypeScript-first schema validation library.',
};
