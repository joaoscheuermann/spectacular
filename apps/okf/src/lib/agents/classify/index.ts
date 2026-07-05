import * as z from 'zod';

import { agent } from '../../agent.js';

const system = `
Classify the file under the following categories:

| File type | What you usually find inside |
| --- | --- |
| Source code | Application or library code, e.g. .ts, .js, .py, .rs, .go, .java, .cpp. |
| Tests | Unit, integration, e2e, snapshot, fixture tests. |
| Configuration | App or tool settings, e.g. .env.example, tsconfig.json, eslint.config.js, pytest.ini. |
| Package/manifest | Dependency and project metadata, e.g. package.json, Cargo.toml, pyproject.toml, pom.xml. |
| Lockfiles | Pinned dependency resolution, e.g. package-lock.json, pnpm-lock.yaml, Cargo.lock. |
| Build | Build system definitions, e.g. Makefile, webpack.config.js, vite.config.ts, build.gradle. |
| Documentation | README.md, CONTRIBUTING.md, architecture docs, API docs, runbooks. |
| Generated | Compiled code, generated clients, protobuf outputs, ORM types, build artifacts. |
| Schema/contract | OpenAPI specs, GraphQL schemas, JSON Schema, protobuf, Avro. |
| Database | Migrations, seed data, SQL scripts, schema snapshots. |
| Scripts | Automation, maintenance, release, setup, dev tooling scripts. |
| CI/CD | GitHub Actions, GitLab CI, CircleCI, Azure Pipelines configs. |
| Infrastructure | Dockerfiles, Compose files, Terraform, Kubernetes manifests, Helm charts. |
| Deployment/runtime | Service configs, process managers, systemd units, platform configs. |
| Static assets | Images, fonts, icons, audio, video, PDFs. |
| Templates | HTML templates, email templates, codegen templates. |
| Localization | Translation catalogs, e.g. .po, .json, .yaml. |
| Data/fixtures | Sample data, mock payloads, golden files, test snapshots. |
| Editor/tooling | .vscode, .editorconfig, formatting/linting configs. |
| Version-control metadata | .gitignore, .gitattributes, CODEOWNERS. |
| Legal/governance | LICENSE, NOTICE, security policy, changelog. |
| Secret-related | Encrypted secrets, secret templates, or references; raw secrets should generally not live in the repo. |
| Binary/vendor | Vendored dependencies, compiled libraries, archives, certificates. |
| None | When the file does not fit in any category described above |

Write clean, direct, neutral language.
Do not use emoji or decorative symbols.
Do not use hype, marketing language, or inflated claims.
Do not hallucinate. Do not invent behavior, intent, relationships, or context.
Do not make assumptions. Use only information present in the supplied facts and content excerpt.
`;

const schema = z.object({
  type: z.array(
    z.enum([
      'source_code',
      'test',
      'configuration',
      'package_manifest',
      'lockfile',
      'build',
      'documentation',
      'generated',
      'schema_contract',
      'database',
      'script',
      'ci_cd',
      'infrastructure',
      'deployment_runtime',
      'static_asset',
      'template',
      'localization',
      'data_fixture',
      'editor_tooling',
      'version_control_metadata',
      'legal_governance',
      'secret_related',
      'binary_vendor',
      'none',
    ]),
  ),
  reason: z.string(),
});

const prompt = (path: string, body: string) => `
  Classify the following file:

  ~~~\`${path}\`
  ${body}
  ~~~
`;

export const classify = (path: string, body: string) =>
  agent('poolside/laguna-xs-2.1', system, 'low').complete(prompt(path, body), {
    schema,
  });
