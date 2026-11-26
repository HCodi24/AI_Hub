import { Html, Head, Main, NextScript } from "next/document";
export default function Document() {
  return <Html lang="vi" data-theme="dark" data-style="default"><Head>
    <script dangerouslySetInnerHTML={{ __html: `(function(){
      try {
        var theme = localStorage.getItem('ai-hub-theme');
        var style = localStorage.getItem('ai-hub-style');
        var dataset = document.documentElement.dataset;
        dataset.theme = theme === 'glass' ? 'light' : ['light', 'dark', 'system'].includes(theme) ? theme : 'dark';
        dataset.style = theme === 'glass' ? 'glass' : ['default', 'glass'].includes(style) ? style : 'default';
      } catch {}
    })()` }} />
    <script src="/app/mutis-picker.js" defer data-same-origin-only={process.env.NODE_ENV === "production" ? "true" : undefined} />
  </Head><body><Main /><NextScript /></body></Html>;
}
