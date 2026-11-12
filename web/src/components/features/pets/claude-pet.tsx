import styles from "./claude-pet.module.css";
// Original pixel critter for Claude replies (12 x 12 grid): a spark on top, a round body, little arms and feet.
const BODY = [
  "....####....",
  "..########..",
  ".##########.",
  ".##########.",
  "############",
  ".##########.",
  ".##########.",
  "..########..",
];
const SPARK = [[6, 0], [5, 1], [6, 1], [7, 1], [6, 2]];
const FEET = [[3, 11], [4, 11], [7, 11], [8, 11]];
const pixels = (rows: string[], top: number) =>
  rows.flatMap((row, y) => [...row].flatMap((cell, x) => cell === "#" ? [[x, y + top]] : []));
export function ClaudePet({ busy }: { busy: boolean }) {
  return <svg className={styles.pet} data-busy={busy} viewBox="0 0 12 12" shapeRendering="crispEdges" aria-hidden="true">
    <g className={styles.spark}>{SPARK.map(([x, y]) => <rect key={`s${x}${y}`} x={x} y={y} width="1" height="1" />)}</g>
    <g className={styles.body}>{[...pixels(BODY, 3), ...FEET].map(([x, y]) => <rect key={`b${x}-${y}`} x={x} y={y} width="1" height="1" />)}
      <g className={styles.eyes}><rect x="4" y="6" width="1" height="2" /><rect x="7" y="6" width="1" height="2" /></g></g>
  </svg>;
}
