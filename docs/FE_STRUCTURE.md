# Quy ước frontend AI Hub

R1 dùng cấu trúc của `C:/GitHub/BEANS/SCM-BEANS_Dashboard_FE`, không sao chép nghiệp vụ. Các bước frontend sau tiếp tục theo cấu trúc này.

- `web/next.config.js`: Next.js 16 Pages Router, React 19, `pageExtensions: ["page.tsx", "page.ts"]`, `basePath: "/app"`, trailing slash, static export `out/`, ảnh không tối ưu qua server.
- `web/tsconfig.json`: strict, bundler resolution, `@/*` ánh xạ `src/*`. Không dùng `any`.
- `src/apis/<name>.api.ts`: một object cho mỗi vùng backend. Dùng client `src/lib/http.ts`, parse dữ liệu unknown bằng Zod trước khi trả cho UI. Không đưa API key vào store.
- `src/schema/<name>.schema.ts`: Zod schemas và kiểu suy ra bằng `z.infer`. Schema đọc lịch sử phải nhận defaults của Rust.
- `src/lib/`: HTTP JSON cùng origin `/api`, `ApiError` tiếng Việt; POST SSE có AbortSignal; markdown (marked, DOMPurify, highlight.js, ẩn memory); `utils.ts` có `cn` từ clsx + twMerge.
- `src/routes/path.ts`, `routes.ts`: hằng đường dẫn và mapping route. Next Link tự thêm basePath; link về UI cũ dùng anchor `/`.
- `src/constants/`: các hằng/enum dùng chung. `src/text/<area>.ts`: mọi nhãn, trạng thái, lỗi UI tiếng Việt. Component không hard-code chuỗi hiển thị.
- `src/components/<kebab-name>/<kebab-name>.tsx` và `<kebab-name>.module.css`: UI dùng chung, named exports, props có kiểu. Ví dụ button, icon-button, select, sidebar, markdown-view, spinner, empty-state. Thêm badge/dialog/code-block khi cần tách thành phần dùng chung; không tạo component rỗng.
- `src/components/features/<feature>/`: UI theo tính năng (chat, composer; sau này debate, plan, memory, settings, mutis, dev). Store chat và hook phối hợp API nằm cạnh feature. Component giữ nhỏ, khoảng dưới 250 dòng.
- `src/pages/_app.page.tsx`: QueryClientProvider, mỗi phiên app có một QueryClient. `index.page.tsx` và `chat/index.page.tsx` là màn hình mỏng. Dùng `/app/chat/` hoặc `/app/` và chọn chat trong sidebar để tương thích static export.
- `src/styles/globals.css`, `tokens.css`: spacing 4/8/12/16/24/32, radius, shadow, font sizes, màu light/dark qua data-theme và prefers-color-scheme. Font hệ thống và mono, không tải font ngoài.

React Query quản lý server data và invalidation. Zustand quản lý selection, messages đang stream, trạng thái UI. SSE xử lý delta/reasoning/log/diff/error/status/heartbeat/memory/done; thiếu done được báo là phản hồi chưa hoàn tất. Nút Dừng gọi API stop của chat rồi hủy reader.

CSS Modules + tokens là quy ước mặc định. Không dùng Tailwind để thay CSS Modules. Icon lấy từ lucide-react; không dùng emoji ở chrome. Label gắn input, focus-visible rõ, drawer có Escape/Tab trap, streaming dùng aria-live. Kiểm tra chiều rộng 375px và cả hai theme.

Build web trước release Rust để nhúng export. Chạy typecheck, lint, build, verify và smoke; tuyệt đối không dùng AI thật trong kiểm tra tự động.
