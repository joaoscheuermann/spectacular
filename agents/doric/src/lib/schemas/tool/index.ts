import z from "zod";
import { CanonicalNameSchema, JsonObjectSchema } from "../general/index.js";

/**
 * The serializable runtime descriptor for one executable tool.
 *
 * The handler and the executable Zod schema remain outside this descriptor.
 */
export const ToolDescriptorSchema = z
  .object({
    name: CanonicalNameSchema.describe(
      'The unique canonical name of the tool. Skill references and tool calls use this exact value.'
    ),

    description: z
      .string()
      .trim()
      .min(1, 'The tool description cannot be empty.')
      .optional()
      .describe(
        'A concise explanation of the operation performed by the tool and when that operation is useful.'
      ),

    inputSchema: JsonObjectSchema.describe(
      'The JSON Schema that defines the input accepted by the tool.'
    ),

    outputSchema: JsonObjectSchema.describe(
      'The JSON Schema that defines the observable result returned by the tool.'
    ),

    strict: z
      .boolean()
      .optional()
      .describe(
        'Whether the tool caller must enforce the input schema without accepting unspecified fields.'
      ),
  })
  .strict();

// /**
//  * A validated catalog of canonical skills and registered tools.
//  *
//  * Enforces:
//  * - unique skill names;
//  * - unique tool names;
//  * - resolution of every allowedTools reference.
//  */
// export const CatalogSchema = z
//   .object({
//     skills: z.array(SkillRecordSchema).describe(
//       'The canonical skills available for retrieval, reranking, and execution.'
//     ),

//     tools: z.array(ToolDescriptorSchema).describe(
//       'The registered tool descriptors available to the executor.'
//     ),
//   })
//   .strict()
//   .superRefine((data, ctx) => {
//     const skillNames = new Map<string, number>();
//     const toolNames = new Map<string, number>();

//     for (const [index, skill] of data.skills.entries()) {
//       const previousIndex = skillNames.get(skill.name);

//       if (previousIndex !== undefined) {
//         ctx.addIssue({
//           code: z.ZodIssueCode.custom,
//           message:
//             `Duplicate skill name "${skill.name}". ` +
//             `It was first declared at skills[${previousIndex}].`,
//           path: ['skills', index, 'name'],
//         });
//       } else {
//         skillNames.set(skill.name, index);
//       }
//     }

//     for (const [index, tool] of data.tools.entries()) {
//       const previousIndex = toolNames.get(tool.name);

//       if (previousIndex !== undefined) {
//         ctx.addIssue({
//           code: z.ZodIssueCode.custom,
//           message:
//             `Duplicate tool name "${tool.name}". ` +
//             `It was first declared at tools[${previousIndex}].`,
//           path: ['tools', index, 'name'],
//         });
//       } else {
//         toolNames.set(tool.name, index);
//       }
//     }

//     const registeredToolNames = new Set(
//       data.tools.map((tool) => tool.name)
//     );

//     for (const [skillIndex, skill] of data.skills.entries()) {
//       for (const [toolIndex, toolName] of skill.allowedTools.entries()) {
//         if (!registeredToolNames.has(toolName)) {
//           ctx.addIssue({
//             code: z.ZodIssueCode.custom,
//             message:
//               `Skill "${skill.name}" references the unregistered tool "${toolName}".`,
//             path: [
//               'skills',
//               skillIndex,
//               'allowedTools',
//               toolIndex,
//             ],
//           });
//         }
//       }
//     }
//   });

// export type Catalog = z.output<
//   typeof CatalogSchema
// >;
