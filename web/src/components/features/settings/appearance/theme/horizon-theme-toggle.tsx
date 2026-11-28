import styles from "./horizon-theme-toggle.module.css";

interface HorizonThemeToggleProps {
  isDark: boolean;
  onToggle: () => void;
  size?: "sm" | "md";
  className?: string;
}

export function HorizonThemeToggle({
  isDark,
  onToggle,
  size = "md",
  className = "",
}: HorizonThemeToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={isDark}
      aria-label={isDark ? "Chuyển sang giao diện sáng" : "Chuyển sang giao diện tối"}
      title={isDark ? "Tối" : "Sáng"}
      onClick={onToggle}
      data-dark={isDark}
      data-size={size}
      className={`${styles.pill} ${className}`.trim()}
    >
      {/* Sun / Moon Orb */}
      <span className={styles.orb} aria-hidden="true" />

      {/* Clouds (Light mode) */}
      <svg
        className={styles.clouds}
        width="24"
        height="14"
        viewBox="0 0 24 14"
        fill="none"
        aria-hidden="true"
      >
        <path
          d="M5 13H19C21.2 13 23 11.2 23 9C23 7 21.5 5.3 19.5 5C18.8 2.2 16.3 0 13.2 0C10.1 0 7.5 2.3 7.1 5.3C4.8 5.6 3 7.5 3 9.8C3 11.6 4 13 5 13Z"
          fill="#FFFFFF"
          fillOpacity="0.9"
        />
      </svg>

      {/* Stars (Dark mode) */}
      <svg
        className={styles.stars}
        width="22"
        height="18"
        viewBox="0 0 22 18"
        fill="none"
        aria-hidden="true"
      >
        <path
          d="M6 2 Q6 6 10 6 Q6 6 6 10 Q6 6 2 6 Q6 6 6 2 Z"
          fill="#8CE0FA"
        />
        <circle cx="15" cy="13" r="1.6" fill="#8CE0FA" opacity="0.85" />
        <circle cx="17" cy="4.5" r="1.1" fill="#FFFFFF" opacity="0.7" />
      </svg>
    </button>
  );
}
