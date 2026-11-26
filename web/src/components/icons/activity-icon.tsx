import type { LucideProps } from "lucide-react";

/** Three stacked layers for Activity, in the MutiS brand colors; each path carries data-layer so CSS can animate it while running. */
export function ActivityIcon({ size = 24, color: _color, strokeWidth: _strokeWidth, absoluteStrokeWidth: _absolute, ...props }: LucideProps) {
  return <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width={size} height={size} fill="none" {...props}>
    <path data-layer="bot" d="M 20 72 L 50 88 L 80 72" stroke="#79DEFD" strokeWidth={8.5} strokeLinecap="round" strokeLinejoin="round" opacity={0.5} />
    <path data-layer="mid" d="M 20 58 L 50 74 L 80 58" stroke="#79DEFD" strokeWidth={8.5} strokeLinecap="round" strokeLinejoin="round" />
    <path data-layer="top" d="M 50 20 L 80 36 L 50 52 L 20 36 Z" fill="#697DFC" stroke="#697DFC" strokeWidth={6} strokeLinejoin="round" />
  </svg>;
}
