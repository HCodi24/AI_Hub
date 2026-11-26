import type { LucideProps } from "lucide-react";

/** Plug with a looped cord for MCP/plugin activity, in the MutiS brand colors like CommandIcon; lucide-compatible props. */
export function PlugIcon({ size = 24, color: _color, strokeWidth: _strokeWidth, absoluteStrokeWidth: _absolute, ...props }: LucideProps) {
  return <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width={size} height={size} fill="none" {...props}>
    <path d="M 62.1 82.4 A 35.5 35.5 0 1 0 41.4 83.45 C 47.0 84.85 50 82.5 50 72 V 66" stroke="#79DEFD" strokeWidth={7.2} strokeLinecap="round" strokeLinejoin="round" />
    <rect x="42.1" y="31.5" width="5.2" height="14" rx="2.6" fill="#697DFC" />
    <rect x="52.7" y="31.5" width="5.2" height="14" rx="2.6" fill="#697DFC" />
    <path d="M 40.2 43 H 59.8 A 3.2 3.2 0 0 1 63 46.2 V 55 A 13 13 0 0 1 37 55 V 46.2 A 3.2 3.2 0 0 1 40.2 43 Z" fill="#697DFC" />
  </svg>;
}
