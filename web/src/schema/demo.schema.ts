import { z } from "zod";

const DemoDirSchema = z.object({
  path: z.string(),
  files: z.number().int().nonnegative(),
  bytes: z.number().int().nonnegative(),
});

export const DemoFilesSchema = z.object({
  dirs: z.array(DemoDirSchema),
  files: z.number().int().nonnegative(),
  bytes: z.number().int().nonnegative(),
});

export const DemoRemoveSchema = DemoFilesSchema.extend({ skipped: z.array(z.string()) });
