import { antigravityModel } from "./antigravity-models";

const capitalize = (name: string) => name[0].toUpperCase() + name.slice(1);

export function modelDisplayLabel(model: string, provider?: string): string {
  if (provider === "codex_cli") {
    return model.replace(/^gpt-(\d+(?:\.\d+)*)-([a-z]+)$/i, (_, version: string, family: string) => `${capitalize(family)}-${version}`);
  }
  if (provider === "antigravity_cli") {
    model = antigravityModel(model).id;
    const gemini = /^gemini-(\d+(?:\.\d+)*)-([a-z]+)$/i.exec(model);
    if (gemini) return `${capitalize(gemini[2])}-${gemini[1]}`;
  }
  return model.replace(/^claude-([a-z]+)-(\d+(?:-\d{1,2})*)$/i, (_, family: string, version: string) =>
    `${capitalize(family)}-${version.replaceAll("-", ".")}`);
}
