import z from "zod";
import { ToolDescriptorSchema } from "../schemas/tool/index.js";

export type ToolDescriptor = z.output<
    typeof ToolDescriptorSchema
>;
