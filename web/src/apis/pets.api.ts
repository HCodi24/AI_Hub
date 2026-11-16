import { http, parseResponse } from "@/lib/http";
import { PetsSchema } from "@/schema/pets.schema";
const id = (value: string) => encodeURIComponent(value);
export const petsApi = {
  async list(signal?: AbortSignal) { return parseResponse(PetsSchema, await http.get("/pets", signal)).pets; },
  /** Copies the pet folder (pet.json + sprite sheet) the user picked into the app. */
  async import(path: string) { await http.request("/pets/import", { method: "POST", body: JSON.stringify({ path }) }); },
  /** Sends the WebP sprite sheet the user picked as the raw request body. */
  async upload(file: File) {
    await http.request(`/pets/upload?name=${id(file.name)}`, { method: "POST", body: file, headers: { "Content-Type": "image/webp" } });
  },
  async remove(pet: string) { await http.request(`/pets/${id(pet)}`, { method: "DELETE" }); },
};
