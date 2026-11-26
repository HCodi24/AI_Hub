import type { LucideProps } from "lucide-react";

/** Two interlocked halves of ⌘ in the MutiS brand colors (web/public/brand/mutis-logo.svg); lucide-compatible props. */
export function CommandIcon({ size = 24, color: _color, strokeWidth: _strokeWidth, absoluteStrokeWidth: _absolute, ...props }: LucideProps) {
  return <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width={size} height={size} fill="none" {...props}>
    <path d="M 37 63 H 26 A 11 11 0 1 0 37 74 V 26 A 11 11 0 1 0 26 37 H 63"
      stroke="#697DFC" strokeWidth={8.5} strokeLinecap="round" strokeLinejoin="round" />
    <path d="M 63 37 H 74 A 11 11 0 1 0 63 26 V 74 A 11 11 0 1 0 74 63 H 37"
      stroke="#79DEFD" strokeWidth={8.5} strokeLinecap="round" strokeLinejoin="round" />
  </svg>;
}
