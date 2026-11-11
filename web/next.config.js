/** @type {import('next').NextConfig} */
module.exports = (phase) => ({
  output: "export",
  ...(phase === "phase-development-server" ? {
    async rewrites() { return [{ source: "/api/:path*", destination: "http://127.0.0.1:7878/api/:path*", basePath: false }]; },
  } : {}),
  basePath: "/app",
  trailingSlash: true,
  images: { unoptimized: true },
  reactStrictMode: true,
  pageExtensions: ["page.tsx", "page.ts"],
});
