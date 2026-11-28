import { Html, Head, Main, NextScript } from "next/document";
export default function Document() {
  return <Html lang="vi" data-theme="dark" data-style="default"><Head>
    <script dangerouslySetInnerHTML={{ __html: `(function(){
      try {
        var theme = localStorage.getItem('ai-hub-theme');
        var style = localStorage.getItem('ai-hub-style');
        var bg = localStorage.getItem('ai-hub-glass-bg');
        var tr = localStorage.getItem('ai-hub-glass-transparency');
        var th = localStorage.getItem('ai-hub-glass-thickness');
        var dataset = document.documentElement.dataset;
        var systemTheme = window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
        dataset.theme = theme === 'glass' ? 'light' : theme === 'system' ? systemTheme : ['light', 'dark'].includes(theme) ? theme : 'dark';
        dataset.style = theme === 'glass' ? 'glass' : ['default', 'glass'].includes(style) ? style : 'default';
        if (bg && bg.indexOf('data:image/') === 0) {
          document.documentElement.style.setProperty('--glass-bg-image', 'url(' + JSON.stringify(bg) + ')');
        }
        if (tr !== null) {
          var n = Number(tr);
          if (isFinite(n)) {
            var clamped = Math.min(100, Math.max(0, Math.round(n)));
            document.documentElement.style.setProperty('--glass-t', (clamped / 100).toFixed(2));
          }
        }
        if (th !== null) {
          var t = Number(th);
          if (isFinite(t)) {
            var clampedTh = Math.min(2, Math.max(0.25, Math.round(t * 20) / 20));
            document.documentElement.style.setProperty('--glass-thickness', clampedTh.toFixed(2));
          }
        }
      } catch {}
    })()` }} />
    <script src="/app/mutis-picker.js" defer data-same-origin-only={process.env.NODE_ENV === "production" ? "true" : undefined} />
  </Head><body><Main /><NextScript /></body></Html>;
}
