import styles from './companion-art.module.css';

export type CompanionPose =
  | 'idle'
  | 'walk'
  | 'happy'
  | 'remember'
  | 'surprised'
  | 'sad'
  | 'sleep';

export interface CompanionArtProps {
  pose: CompanionPose;
  facing: 'left' | 'right';
}

/**
 * Self-contained inline SVG front-facing cloud-head robot mascot.
 * Flat SVG shapes only; poses and facing mirror are driven purely by CSS via
 * `data-pose` and `data-facing` on the root `<svg>`.
 */
export function CompanionArt({ pose, facing }: CompanionArtProps) {
  return (
    <svg
      className={styles.root}
      viewBox="0 0 64 64"
      fill="none"
      focusable="false"
      aria-hidden="true"
      data-pose={pose}
      data-facing={facing}
    >
      {/* Ground shadow */}
      <ellipse className={styles.shadow} cx="32.5" cy="58" rx="13.5" ry="2.6" />

      {/* Whole-character rig (mirrored via scaleX(-1) when data-facing='left') */}
      <g className={styles.rig}>
        {/* Two small rounded feet */}
        <g className={styles.footLeft}>
          <rect className={styles.inkFill} x="24.2" y="48.5" width="8.2" height="9.2" rx="3.4" />
          <rect className={styles.bodyDarkFill} x="25.6" y="49.2" width="5.4" height="7.1" rx="2.3" />
          <rect className={styles.bodyFill} x="25.6" y="49" width="5.4" height="5.8" rx="2.1" />
        </g>

        <g className={styles.footRight}>
          <rect className={styles.inkFill} x="32.6" y="48.5" width="8.2" height="9.2" rx="3.4" />
          <rect className={styles.bodyDarkFill} x="34" y="49.2" width="5.4" height="7.1" rx="2.3" />
          <rect className={styles.bodyFill} x="34" y="49" width="5.4" height="5.8" rx="2.1" />
        </g>

        {/* Two short stubby arms */}
        <g className={styles.armLeft}>
          <path
            className={styles.inkFill}
            d="M 24.8 40.2 C 21.5 40.5 18.4 44.2 18.2 48.2 C 18 51.4 20.4 53.3 23.2 52.6 C 25.4 52 26.8 49.4 27 46.2 Z"
          />
          <path
            className={styles.bodyDarkFill}
            d="M 24.6 41.6 C 22.1 42 19.7 45 19.6 48.1 C 19.5 50.4 21.1 51.7 23 51.2 C 24.6 50.8 25.6 48.6 25.8 46 Z"
          />
          <path
            className={styles.bodyFill}
            d="M 24.4 41.8 C 22.4 42.3 20.4 44.9 20.3 47.6 C 20.2 49.3 21.4 50.2 22.8 49.9 C 24.1 49.6 25 47.8 25.2 45.6 Z"
          />
        </g>

        <g className={styles.armRight}>
          <path
            className={styles.inkFill}
            d="M 40.2 40.2 C 43.5 40.5 46.6 44.2 46.8 48.2 C 47 51.4 44.6 53.3 41.8 52.6 C 39.6 52 38.2 49.4 38 46.2 Z"
          />
          <path
            className={styles.bodyDarkFill}
            d="M 40.4 41.6 C 42.9 42 45.3 45 45.4 48.1 C 45.5 50.4 43.9 51.7 42 51.2 C 40.4 50.8 39.4 48.6 39.2 46 Z"
          />
          <path
            className={styles.bodyFill}
            d="M 40.6 41.8 C 42.6 42.3 44.6 44.9 44.7 47.6 C 44.8 49.3 43.6 50.2 42.2 49.9 C 40.9 49.6 40 47.8 39.8 45.6 Z"
          />
        </g>

        {/* Small round body with chest '>_' terminal prompt glyph */}
        <g className={styles.body}>
          <rect className={styles.inkFill} x="23" y="39.5" width="19" height="13.5" rx="6.5" />
          <rect className={styles.bodyDarkFill} x="24.5" y="40.5" width="16" height="11.1" rx="5.3" />
          <rect className={styles.bodyFill} x="24.5" y="40.3" width="16" height="9.8" rx="4.9" />
          <ellipse className={styles.bodyLightFill} cx="32.8" cy="44.2" rx="6.2" ry="3.4" />

          {/* Counter-flipped when data-facing='left' so '>_' always reads left-to-right */}
          <g className={styles.chestGlyph}>
            <path className={styles.chestStroke} d="M 29.8 44.2 L 32.2 46 L 29.8 47.8" />
            <line className={styles.chestStroke} x1="33.8" y1="47.4" x2="36.8" y2="47.4" />
          </g>
        </g>

        {/* Cloud-shaped head with rounded lobes and dark navy visor */}
        <g className={styles.head}>
          {/* Top-left cloud lobe / bump */}
          <g className={styles.antennaOrBump}>
            <ellipse className={styles.inkFill} cx="28.8" cy="19.2" rx="8.8" ry="8" />
            <ellipse className={styles.bodyFill} cx="28.8" cy="19.2" rx="7.2" ry="6.4" />
            <ellipse className={styles.bodyLightFill} cx="28.8" cy="18.6" rx="6" ry="5" />
          </g>

          {/* Outer dark-navy silhouette of the cloud lobes */}
          <ellipse className={styles.inkFill} cx="38" cy="20.4" rx="8.1" ry="7.4" />
          <ellipse className={styles.inkFill} cx="20.2" cy="25.5" rx="8" ry="7.8" />
          <ellipse className={styles.inkFill} cx="21.6" cy="33.5" rx="7.6" ry="7.2" />
          <ellipse className={styles.inkFill} cx="43.8" cy="27.2" rx="7.8" ry="7.4" />
          <ellipse className={styles.inkFill} cx="42.5" cy="34.2" rx="7.6" ry="6.8" />
          <rect className={styles.inkFill} x="19" y="25" width="26.5" height="16.8" rx="8.2" />

          {/* Lower shading layer of the cloud head */}
          <ellipse className={styles.bodyDarkFill} cx="21.6" cy="33.6" rx="6.1" ry="5.7" />
          <ellipse className={styles.bodyDarkFill} cx="42.5" cy="34.2" rx="6.1" ry="5.3" />
          <rect className={styles.bodyDarkFill} x="20.4" y="26.5" width="23.8" height="13.8" rx="6.8" />

          {/* Main mid-blue cloud lobes */}
          <ellipse className={styles.bodyFill} cx="28.8" cy="19.4" rx="7.1" ry="6.2" />
          <ellipse className={styles.bodyFill} cx="38" cy="20.4" rx="6.5" ry="5.8" />
          <ellipse className={styles.bodyFill} cx="20.2" cy="25.5" rx="6.4" ry="6.2" />
          <ellipse className={styles.bodyFill} cx="21.6" cy="32.8" rx="6" ry="5.4" />
          <ellipse className={styles.bodyFill} cx="43.8" cy="27" rx="6.2" ry="5.8" />
          <ellipse className={styles.bodyFill} cx="42.5" cy="33.4" rx="6" ry="5" />
          <rect className={styles.bodyFill} x="20.5" y="24" width="23.5" height="15.2" rx="7" />

          {/* Soft upper highlight lobes */}
          <ellipse className={styles.bodyLightFill} cx="21.2" cy="24.2" rx="4.9" ry="4.3" />
          <ellipse className={styles.bodyLightFill} cx="28.8" cy="18.6" rx="5.8" ry="4.7" />
          <ellipse className={styles.bodyLightFill} cx="38" cy="19.8" rx="5.3" ry="4.3" />
          <ellipse className={styles.bodyLightFill} cx="43.2" cy="25.8" rx="4.5" ry="4" />
          <rect className={styles.bodyLightFill} x="23" y="19.2" width="19" height="7.5" rx="3.7" />

          {/* Dark navy rounded-rectangle visor and pose-driven expressions */}
          <g className={styles.visor}>
            <rect className={styles.inkFill} x="22.8" y="22.2" width="22.4" height="16.2" rx="5.2" />
            <rect className={styles.visorScreen} x="24.1" y="23.5" width="19.8" height="13.6" rx="4" />

            {/* Soft blush dots (shown in happy pose) */}
            <ellipse className={styles.blush} cx="27.2" cy="33.4" rx="1.7" ry="0.95" />
            <ellipse className={styles.blush} cx="40.8" cy="33.4" rx="1.7" ry="0.95" />

            {/* idle / walk / remember: closed curved glowing cyan eyes */}
            <g className={styles.eyesNormal}>
              <path className={styles.eyeGlowStroke} d="M 27.6 29.6 Q 29.8 32.1 32 29.6" />
              <path className={styles.eyeGlowStroke} d="M 36 29.6 Q 38.2 32.1 40.4 29.6" />
            </g>

            {/* happy: raised ^ ^ arcs + small smile line */}
            <g className={styles.eyesHappy}>
              <path className={styles.eyeGlowStroke} d="M 27.5 30.6 Q 29.8 27.2 32.1 30.6" />
              <path className={styles.eyeGlowStroke} d="M 35.9 30.6 Q 38.2 27.2 40.5 30.6" />
              <path className={styles.mouthGlowStroke} d="M 32.4 33 Q 34 34.6 35.6 33" />
            </g>

            {/* surprised: open round glowing eyes + small 'o' mouth */}
            <g className={styles.eyesSurprised}>
              <circle className={styles.eyeGlowFill} cx="29.8" cy="29.5" r="2.2" />
              <circle className={styles.eyeSpark} cx="30.5" cy="28.7" r="0.7" />
              <circle className={styles.eyeGlowFill} cx="38.2" cy="29.5" r="2.2" />
              <circle className={styles.eyeSpark} cx="38.9" cy="28.7" r="0.7" />
              <circle className={styles.mouthGlowStroke} cx="34" cy="33.7" r="1.35" />
            </g>

            {/* sad: drooping arcs + small frown + tear drop */}
            <g className={styles.eyesSad}>
              <path className={styles.eyeGlowStroke} d="M 27.6 30.8 Q 29.8 29.2 32 29.8" />
              <path className={styles.eyeGlowStroke} d="M 36 29.8 Q 38.2 29.2 40.4 30.8" />
              <path className={styles.mouthGlowStroke} d="M 32.6 34 Q 34 32.8 35.4 34" />
              <path
                className={styles.sadTear}
                d="M 40.4 31.4 C 40.4 31.4 41.6 33.2 41.6 34 A 1.2 1.2 0 1 1 39.2 34 C 39.2 33.2 40.4 31.4 40.4 31.4 Z"
              />
            </g>

            {/* sleep: flat resting lines + tiny resting mouth */}
            <g className={styles.eyesSleep}>
              <line className={styles.eyeGlowStroke} x1="27.7" y1="30.5" x2="31.9" y2="30.5" />
              <line className={styles.eyeGlowStroke} x1="36.1" y1="30.5" x2="40.3" y2="30.5" />
              <ellipse className={styles.mouthGlowStroke} cx="34" cy="33.6" rx="1.05" ry="0.75" />
            </g>
          </g>
        </g>
      </g>

      {/* Overlay badges live outside .rig so they stay readable regardless of facing */}
      {/* remember: glowing lightbulb + sparkle rays above head */}
      <g className={styles.rememberBadge}>
        <circle className={styles.bulbGlass} cx="33" cy="6.2" r="3.4" />
        <circle className={styles.eyeSpark} cx="32" cy="5.2" r="0.95" />
        <rect className={styles.bulbBase} x="31.4" y="9.4" width="3.2" height="1.9" rx="0.8" />
        <line className={styles.bulbRay} x1="33" y1="0.4" x2="33" y2="1.8" />
        <line className={styles.bulbRay} x1="27.6" y1="2.4" x2="28.8" y2="3.5" />
        <line className={styles.bulbRay} x1="38.4" y1="2.4" x2="37.2" y2="3.5" />
      </g>

      {/* surprised: '!' mark above head */}
      <g className={styles.surprisedBadge}>
        <rect className={styles.exclaimMark} x="31.6" y="0.8" width="2.8" height="6.8" rx="1.4" />
        <circle className={styles.exclaimMark} cx="33" cy="9.9" r="1.45" />
      </g>

      {/* sleep: floating 'z' marks above head */}
      <g className={styles.sleepMarks}>
        <g className={styles.sleepZ1}>
          <path
            className={styles.sleepZStroke}
            strokeWidth="1.5"
            d="M 43.5 9.8 H 47 L 43.5 13.2 H 47"
          />
        </g>
        <g className={styles.sleepZ2}>
          <path
            className={styles.sleepZStroke}
            strokeWidth="1.65"
            d="M 47.8 5.2 H 52.2 L 47.8 9.4 H 52.2"
          />
        </g>
        <g className={styles.sleepZ3}>
          <path
            className={styles.sleepZStroke}
            strokeWidth="1.8"
            d="M 52.5 0.8 H 57.5 L 52.5 5.4 H 57.5"
          />
        </g>
      </g>
    </svg>
  );
}
