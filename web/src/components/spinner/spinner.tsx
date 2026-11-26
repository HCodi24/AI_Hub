import { chatText } from "@/text/chat";
import styles from "./spinner.module.css";

export function Spinner() {
  return (
    <span role="status" className={styles.spinner}>
      <svg width={48} height={48} viewBox="0 0 100 100" fill="none" aria-hidden="true">
        <ellipse cx={50} cy={50} rx={40} ry={17} transform="rotate(45 50 50)" stroke="#6E7BF2" strokeWidth={5} opacity={0.18} />
        <ellipse cx={50} cy={50} rx={40} ry={17} transform="rotate(-45 50 50)" stroke="#8CE0FA" strokeWidth={5} opacity={0.22} />
        <ellipse className={styles.orbitA} cx={50} cy={50} rx={40} ry={17} transform="rotate(45 50 50)" stroke="#6E7BF2" strokeWidth={7} strokeLinecap="round" />
        <ellipse className={styles.orbitB} cx={50} cy={50} rx={40} ry={17} transform="rotate(-45 50 50)" stroke="#8CE0FA" strokeWidth={7} strokeLinecap="round" />
        <path className={styles.core} d="M 50 38 L 62 50 L 50 62 L 38 50 Z" fill="#6E7BF2" />
      </svg>
      <span className={styles["visually-hidden"]}>{chatText.loading}</span>
    </span>
  );
}
