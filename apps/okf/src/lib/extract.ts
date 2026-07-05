import path from 'node:path';

import type {
  CodeExtraction,
  Declaration,
  DescribedValue,
  FileExtraction,
  FileKind,
  PackageExtraction,
} from './types.js';
import { normalizeRelativePath } from './paths.js';
import { extractConfigKeys } from './config.js';
import {
  describeDevelopmentDependency,
  describeRuntimeDependency,
} from './dependencies.js';

const codeExtensions = new Set([
  '.cjs',
  '.go',
  '.js',
  '.jsx',
  '.mjs',
  '.py',
  '.rs',
  '.ts',
  '.tsx',
]);

const configurationNames = new Set([
  '.eslintrc',
  '.prettierrc',
  'dockerfile',
  'package-lock.json',
  'project.json',
  'tsconfig.json',
]);

const configurationExtensions = new Set([
  '.json',
  '.jsonc',
  '.toml',
  '.yaml',
  '.yml',
]);

/** Extracts deterministic facts that every generated OKF concept should preserve. */
export const extractFileFacts = (
  relativePath: string,
  content: string,
): { readonly extraction: FileExtraction; readonly kind: FileKind } => {
  const basename = path.posix.basename(normalizeRelativePath(relativePath));
  const extension = path.posix.extname(basename).toLowerCase();

  if (basename === 'package.json') {
    return packageFacts(relativePath, content);
  }

  if (codeExtensions.has(extension)) {
    return codeFacts(relativePath, content, extension);
  }

  if (
    configurationExtensions.has(extension) ||
    configurationNames.has(basename.toLowerCase())
  ) {
    return configurationFacts(relativePath, content);
  }

  return textFacts(relativePath);
};

const packageFacts = (
  relativePath: string,
  content: string,
): { readonly extraction: FileExtraction; readonly kind: FileKind } => {
  const parsed = parseJson(content);
  const manifest: PackageExtraction = {
    scripts: describedRecord(recordField(parsed, 'scripts'), describeScript),
    dependencies: describedRecord(
      recordField(parsed, 'dependencies'),
      describeRuntimeDependency,
    ),
    devDependencies: describedRecord(
      recordField(parsed, 'devDependencies'),
      describeDevelopmentDependency,
    ),
  };

  return {
    kind: 'package-config',
    extraction: {
      type: 'Package Configuration',
      title: relativePath,
      description: `Package manifest and npm workspace configuration for ${relativePath}.`,
      tags: ['configuration', 'package-json'],
      package: manifest,
      configKeys: objectKeys(parsed),
      relationships: [],
    },
  };
};

const configurationFacts = (
  relativePath: string,
  content: string,
): { readonly extraction: FileExtraction; readonly kind: FileKind } => {
  const keys = extractConfigKeys(relativePath, content);

  return {
    kind: 'configuration',
    extraction: {
      type: 'Configuration File',
      title: relativePath,
      description:
        keys.length === 0
          ? `Configuration file ${relativePath}.`
          : `Configuration file ${relativePath} with keys ${keys.join(', ')}.`,
      tags: ['configuration'],
      configKeys: keys,
      relationships: [],
    },
  };
};

const codeFacts = (
  relativePath: string,
  content: string,
  extension: string,
): { readonly extraction: FileExtraction; readonly kind: FileKind } => {
  const code = extractCode(content, extension);

  return {
    kind: 'code',
    extraction: {
      type: 'Source Code',
      title: relativePath,
      description: `Source code file ${relativePath}.`,
      tags: ['code', extension.replace('.', '')],
      code,
      relationships: relationshipSpecifiers(code.imports, extension).map(
        (specifier) => ({ kind: 'imports', specifier }),
      ),
    },
  };
};

const textFacts = (
  relativePath: string,
): { readonly extraction: FileExtraction; readonly kind: FileKind } => ({
  kind: 'text',
  extraction: {
    type: 'Text Document',
    title: relativePath,
    description: `Text document ${relativePath}.`,
    tags: ['text'],
    relationships: [],
  },
});

const extractCode = (content: string, extension: string): CodeExtraction => {
  if (extension === '.py') {
    return pythonExtraction(content);
  }

  if (extension === '.rs') {
    return rustExtraction(content);
  }

  return jsExtraction(content);
};

const jsExtraction = (content: string): CodeExtraction => {
  const imports = unique([
    ...captures(content, /\bimport\s+(?:[^'"]+\s+from\s+)?['"]([^'"]+)['"]/g),
    ...captures(content, /\bexport\s+[^'"]+\s+from\s+['"]([^'"]+)['"]/g),
    ...captures(content, /\brequire\(\s*['"]([^'"]+)['"]\s*\)/g),
  ]);

  return {
    imports,
    exports: jsExports(content),
    declarations: uniqueDeclarations([
      ...declarations(
        content,
        /\bfunction\s+([A-Za-z_$][\w$]*)\s*\(/g,
        'function',
      ),
      ...declarations(
        content,
        /\binterface\s+([A-Za-z_$][\w$]*)\b/g,
        'interface',
      ),
      ...declarations(content, /\bclass\s+([A-Za-z_$][\w$]*)\b/g, 'class'),
      ...declarations(content, /\btype\s+([A-Za-z_$][\w$]*)\b/g, 'type'),
      ...declarations(
        content,
        /\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=/g,
        'value',
      ),
    ]),
    methods: jsMethods(content),
  };
};

const jsExports = (content: string): readonly Declaration[] =>
  uniqueDeclarations([
    ...declarations(
      content,
      /\bexport\s+(?:async\s+)?function\s+([A-Za-z_$][\w$]*)\s*\(/g,
      'function',
    ),
    ...declarations(
      content,
      /\bexport\s+interface\s+([A-Za-z_$][\w$]*)\b/g,
      'interface',
    ),
    ...declarations(
      content,
      /\bexport\s+class\s+([A-Za-z_$][\w$]*)\b/g,
      'class',
    ),
    ...declarations(content, /\bexport\s+type\s+([A-Za-z_$][\w$]*)\b/g, 'type'),
    ...declarations(
      content,
      /\bexport\s+(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=/g,
      'value',
    ),
    ...captures(content, /\bexport\s*\{([^}]+)\}/g).flatMap(namedExports),
  ]);

const namedExports = (value: string): readonly Declaration[] =>
  value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => ({
      kind: 'export',
      name:
        item
          .split(/\s+as\s+/u)
          .at(-1)
          ?.trim() ?? item,
    }));

const jsMethods = (content: string): readonly string[] =>
  unique(
    captures(
      content,
      /^\s*(?:public|private|protected|static|async|override|get|set\s+)*([A-Za-z_$][\w$]*)\s*\([^)]*\)\s*(?::[^{]+)?\{/gm,
    ).filter((name) => !['for', 'if', 'switch', 'while'].includes(name)),
  );

const pythonExtraction = (content: string): CodeExtraction => ({
  imports: pythonImports(content),
  exports: declarations(content, /^([A-Za-z_][\w]*)\s*=/gm, 'value').filter(
    (item) => !item.name.startsWith('_'),
  ),
  declarations: uniqueDeclarations([
    ...declarations(content, /^\s*def\s+([A-Za-z_][\w]*)\s*\(/gm, 'function'),
    ...declarations(content, /^\s*class\s+([A-Za-z_][\w]*)\s*[:(]/gm, 'class'),
  ]),
  methods: unique(captures(content, /^\s+def\s+([A-Za-z_][\w]*)\s*\(/gm)),
});

const rustExtraction = (content: string): CodeExtraction => ({
  imports: unique([
    ...captures(content, /^\s*use\s+([^;]+);/gm).flatMap(rustUseSpecifiers),
    ...captures(content, /^\s*mod\s+([A-Za-z_][\w]*);/gm),
  ]),
  exports: uniqueDeclarations([
    ...declarations(content, /\bpub\s+fn\s+([A-Za-z_][\w]*)\s*\(/g, 'function'),
    ...declarations(content, /\bpub\s+struct\s+([A-Za-z_][\w]*)\b/g, 'struct'),
    ...declarations(content, /\bpub\s+enum\s+([A-Za-z_][\w]*)\b/g, 'enum'),
    ...declarations(content, /\bpub\s+trait\s+([A-Za-z_][\w]*)\b/g, 'trait'),
  ]),
  declarations: uniqueDeclarations([
    ...declarations(content, /\bfn\s+([A-Za-z_][\w]*)\s*\(/g, 'function'),
    ...declarations(content, /\bstruct\s+([A-Za-z_][\w]*)\b/g, 'struct'),
    ...declarations(content, /\benum\s+([A-Za-z_][\w]*)\b/g, 'enum'),
    ...declarations(content, /\btrait\s+([A-Za-z_][\w]*)\b/g, 'trait'),
  ]),
  methods: unique(captures(content, /\bfn\s+([A-Za-z_][\w]*)\s*\(/g)),
});

const describedRecord = (
  record: Record<string, unknown> | undefined,
  describe: (name: string, value: string) => string,
): readonly DescribedValue[] =>
  Object.entries(record ?? {}).map(([name, value]) => {
    const text = String(value);

    return {
      name,
      value: text,
      description: describe(name, text),
    };
  });

const describeScript = (name: string, command: string): string => {
  if (command.includes('nx')) {
    return `Runs the Nx-backed ${name} workflow.`;
  }

  if (command.includes('tsc')) {
    return `Runs the TypeScript ${name} workflow.`;
  }

  if (command.includes('test')) {
    return `Runs the ${name} test workflow.`;
  }

  return `Runs the ${name} script command.`;
};

const parseJson = (content: string): Record<string, unknown> | undefined => {
  try {
    const value = JSON.parse(content) as unknown;

    return isRecord(value) ? value : undefined;
  } catch {
    return undefined;
  }
};

const relationshipSpecifiers = (
  imports: readonly string[],
  extension: string,
): readonly string[] => {
  if (extension === '.py' || extension === '.rs') {
    return imports;
  }

  return imports.filter((specifier) => specifier.startsWith('.'));
};

const pythonImports = (content: string): readonly string[] =>
  unique([
    ...captures(content, /^\s*import\s+([^\n#]+)/gm)
      .flatMap((value) => value.split(','))
      .map((value) => pythonImportName(value))
      .filter((value): value is string => value !== undefined),
    ...[
      ...content.matchAll(/^\s*from\s+([A-Za-z0-9_.]+)\s+import\s+([^\n#]+)/gm),
    ].flatMap((match) => pythonFromImports(match[1] ?? '', match[2] ?? '')),
  ]);

const pythonImportName = (value: string): string | undefined => {
  const name = value
    .trim()
    .split(/\s+as\s+/u)[0]
    ?.trim();

  return name === undefined || name === '' ? undefined : name;
};

const pythonFromImports = (
  moduleName: string,
  importedNames: string,
): readonly string[] => {
  const module = moduleName.trim();

  if (/^\.+$/u.test(module)) {
    return importedNames
      .split(',')
      .map((value) => pythonImportName(value))
      .filter((value): value is string => value !== undefined)
      .map((name) => `${module}${name}`);
  }

  return [module];
};

const rustUseSpecifiers = (value: string): readonly string[] => {
  const specifier = value.trim();

  if (!specifier.includes('{')) {
    return [specifier];
  }

  const match = /^(.*)::\{([^}]+)\}$/u.exec(specifier);

  if (match === null) {
    return [specifier];
  }

  return match[2]
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => `${match[1]}::${item}`);
};

const recordField = (
  record: Record<string, unknown> | undefined,
  key: string,
): Record<string, unknown> | undefined => {
  const value = record?.[key];

  return isRecord(value) ? value : undefined;
};

const objectKeys = (
  record: Record<string, unknown> | undefined,
): readonly string[] => Object.keys(record ?? {});

const declarations = (
  content: string,
  pattern: RegExp,
  kind: string,
): readonly Declaration[] =>
  captures(content, pattern).map((name) => ({
    kind,
    name,
  }));

const captures = (content: string, pattern: RegExp): readonly string[] =>
  [...content.matchAll(pattern)]
    .map((match) => match[1]?.trim())
    .filter((value): value is string => value !== undefined && value !== '');

const unique = (values: readonly string[]): readonly string[] => [
  ...new Set(values),
];

const uniqueDeclarations = (
  values: readonly Declaration[],
): readonly Declaration[] => {
  const seen = new Set<string>();

  return values.filter((value) => {
    const key = `${value.kind}:${value.name}`;

    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === 'object' && !Array.isArray(value);
