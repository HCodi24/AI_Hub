import { Html, Head, Main, NextScript } from "next/document";
export default function Document() {
  return <Html lang="vi"><Head><script dangerouslySetInnerHTML={{ __html: `(function(){try{var t=localStorage.getItem('ai-hub-theme');document.documentElement.dataset.theme=['light','dark','system'].includes(t)?t:'dark'}catch{}})()` }} /></Head><body><Main /><NextScript /></body></Html>;
}
