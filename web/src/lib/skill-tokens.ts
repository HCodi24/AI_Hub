import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { addonsApi } from "@/apis/addons.api";

export const UNICODE_WHITESPACE = /^\p{White_Space}$/u;
type HighlightPart = { text: string; highlighted: boolean };

export function skillHighlightParts(value: string, knownSkills: ReadonlySet<string>): HighlightPart[] {
  const parts: HighlightPart[] = [];
  const tokenPattern = /\/([^\p{White_Space}]*)/gu;
  let cursor = 0;
  for (const match of value.matchAll(tokenPattern)) {
    const start = match.index;
    const token = match[1];
    const end = start + match[0].length;
    const hasBoundary = start === 0 || UNICODE_WHITESPACE.test(value[start - 1]);
    if (!hasBoundary || token.includes("/") || token.includes("\\") || !knownSkills.has(token)) continue;
    if (cursor < start) parts.push({ text: value.slice(cursor, start), highlighted: false });
    parts.push({ text: match[0], highlighted: true });
    cursor = end;
  }
  if (cursor < value.length || parts.length === 0) parts.push({ text: value.slice(cursor), highlighted: false });
  return parts;
}

export function useKnownSkills(): ReadonlySet<string> {
  const addons = useQuery({ queryKey: ["addons"], queryFn: ({ signal }) => addonsApi.overview(signal) });
  const { shared_skills: sharedSkills, builtin_skills: builtinSkills } =
    (addons.isSuccess ? addons.data : undefined) ?? {};
  return useMemo(
    () => new Set<string>([...(sharedSkills ?? []), ...(builtinSkills ?? [])]),
    [sharedSkills, builtinSkills],
  );
}
