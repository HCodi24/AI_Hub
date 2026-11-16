import { z } from "zod";
export const PetSchema = z.object({ id: z.string(), name: z.string() });
export const PetsSchema = z.object({ pets: z.array(PetSchema) });
export type Pet = z.infer<typeof PetSchema>;
