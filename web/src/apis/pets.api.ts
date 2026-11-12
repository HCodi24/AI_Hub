import { http, parseResponse } from "@/lib/http";
import { PetsSchema } from "@/schema/pets.schema";
const id = (value: string) => encodeURIComponent(value);
export const petsApi = {
  async list(signal?: AbortSignal) { return parseResponse(PetsSchema, await http.get("/pets", signal)).pets; },
  async install(pet: string) { await http.request(`/pets/${id(pet)}/install`, { method: "POST" }); },
  /** Copies the pet folder (pet.json + sprite sheet) the user picked into the app. */
  async import(path: string) { await http.request("/pets/import", { method: "POST", body: JSON.stringify({ path }) }); },
  async remove(pet: string) { await http.request(`/pets/${id(pet)}`, { method: "DELETE" }); },
};
