import { Html, Head, Main, NextScript } from "next/document";
export default function Document() {
  return <Html lang="vi" data-theme="dark" data-style="default"><Head>
    <script dangerouslySetInnerHTML={{ __html: `(function(){
      try {
        var theme = localStorage.getItem('ai-hub-theme');
        var style = localStorage.getItem('ai-hub-style');
        var bg = localStorage.getItem('ai-hub-glass-bg');
        var tr = localStorage.getItem('ai-hub-glass-transparency');
        var dataset = document.documentElement.dataset;
        dataset.theme = theme === 'glass' ? 'light' : ['light', 'dark', 'system'].includes(theme) ? theme : 'dark';
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
      } catch {}
    })()` }} />
    <script src="/app/mutis-picker.js" defer data-same-origin-only={process.env.NODE_ENV === "production" ? "true" : undefined} />
  </Head><body><Main /><NextScript /></body></Html>;
}
