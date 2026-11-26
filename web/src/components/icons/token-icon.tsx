import type { LucideProps } from "lucide-react";

/** Coin ring with a holed diamond for token reports, in the MutiS brand colors like CommandIcon; lucide-compatible props. */
export function TokenIcon({ size = 24, color: _color, strokeWidth: _strokeWidth, absoluteStrokeWidth: _absolute, ...props }: LucideProps) {
  return <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width={size} height={size} fill="none" {...props}>
    <circle cx="50" cy="50" r="34" stroke="#79DEFD" strokeWidth={7.2} />
    <path fillRule="evenodd" clipRule="evenodd" fill="#697DFC"
      d="M 46.5 33.5 Q 50 30 53.5 33.5 L 66.5 46.5 Q 70 50 66.5 53.5 L 53.5 66.5 Q 50 70 46.5 66.5 L 33.5 53.5 Q 30 50 33.5 46.5 Z M 55.5 50 A 5.5 5.5 0 1 0 44.5 50 A 5.5 5.5 0 1 0 55.5 50 Z" />
  </svg>;
}
