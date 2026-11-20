export const previewText = {
  server: (port: number, running: boolean) => `Máy chủ xem trước :${port} ${running ? "đang chạy" : "đang tắt"}`,
  start: "Bật", stop: "Tắt", leftover: "(chạy từ trước, không do phiên này bật)", starting: "Đang bật…",
  toggle: "Xem trước", resize: "Đổi độ rộng xem trước",close: "Đóng xem trước", reload: "Tải lại", url: "Địa chỉ trang cần xem",
  placeholder: "localhost:3000/dashboard hoặc localhost:7880/app/preview/?c=settings&tab=mutis",
  empty: "Nhập địa chỉ trang đang sửa. Muốn đổi theo code không cần build thì dùng dev server (MutiS: npm run dev trong web, cổng 7880).", invalid: "Chỉ mở được địa chỉ http hoặc https.",
};
