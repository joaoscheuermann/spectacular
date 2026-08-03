import z from "zod";

/**
 * A JSON-compatible value.
 */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

/**
 * Validates values that can be serialized as JSON.
 */
export const JsonValueSchema: z.ZodType<JsonValue> = z.lazy(() =>
  z.union([
    z.string(),
    z.number().finite(),
    z.boolean(),
    z.null(),
    z.array(JsonValueSchema),
    z.record(z.string(), JsonValueSchema),
  ])
);

/**
 * A JSON-compatible object.
 */
export const JsonObjectSchema = z.record(
  z.string(),
  JsonValueSchema
);

/**
 * A non-empty canonical name.
 *
 * Name uniqueness is enforced at the catalog level.
 */
export const CanonicalNameSchema = z
  .string()
  .trim()
  .min(1, 'The name cannot be empty.');
