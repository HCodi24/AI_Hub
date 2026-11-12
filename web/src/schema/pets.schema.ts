import { z } from "zod";
export const PetSchema = z.object({
  id: z.string(), name: z.string(), provider: z.string(), source: z.string(), notice: z.string(),
  installed: z.boolean(), removable: z.boolean(), custom: z.boolean().default(false),
});
export const PetsSchema = z.object({ pets: z.array(PetSchema) });
export type Pet = z.infer<typeof PetSchema>;
