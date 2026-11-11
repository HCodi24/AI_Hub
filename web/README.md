# AI Hub web

Frontend React 19, Next.js 16 Pages Router và TypeScript strict. Quy ước: [FE_STRUCTURE](../docs/FE_STRUCTURE.md).

Dependencies đã có trong `node_modules`; không cần tải font hay cài thêm package.

```powershell
cd web
npm.cmd run typecheck
npm.cmd run lint
npm.cmd run build
```

Static export nằm tại `web/out`. Chạy backend Rust rồi mở `http://127.0.0.1:7878/app/`.
Build web trước khi build release Rust để nhúng giao diện vào exe. Nếu chưa build web, backend vẫn chạy và `/app/` hiển thị hướng dẫn build.

Phát triển: `npm.cmd run dev`, mở `http://localhost:7880/app/`. Dev server chuyển tiếp `/api/*` tới backend local ở cổng 7878; chạy backend riêng. Bản export dùng API cùng origin, không phụ thuộc dev server.

UI cũ ở `/`. Bản R1 có chat và history; plan/debate mở qua liên kết về UI cũ. Thiết lập nguồn AI thực hiện ở UI cũ.
Không gọi AI khi kiểm tra tự động: `node scripts/test-web.cjs` từ repo root dùng SSE giả lập.
