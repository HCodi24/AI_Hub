export const previewText = {
  server: (port: number) => `localhost:${port}`,
  start: "Bật", stop: "Tắt", starting: "Đang bật…",
  toggle: "Xem trước", resize: "Đổi độ rộng xem trước",close: "Đóng xem trước", reload: "Tải lại", url: "Địa chỉ trang cần xem",
  placeholder: "localhost:3000/dashboard hoặc localhost:7880/app/preview/?c=settings&tab=mutis",
  pick: "Chọn phần tử để gửi cho AI", picking: "Bấm vào phần tử trên trang (Esc để thôi)", noPicker: "Trang này chưa nạp mutis-picker.js nên chưa chọn được phần tử",
  selectedElements: "Phần tử đã chọn", removeElement: "Gỡ phần tử",
  picked: (data: { url: string; selector: string; text: string; css: Record<string, string>; html: string }) => [
    `Phần tử trên trang ${data.url}`, `- Selector: ${data.selector}`, data.text && `- Chữ: ${data.text}`,
    `- CSS: ${Object.entries(data.css).map(([name, value]) => `${name}: ${value}`).join("; ")}`, "```html", data.html, "```", "",
  ].filter(Boolean).join("\n"),
  empty: "Nhập địa chỉ trang đang sửa. Muốn đổi theo code không cần build thì dùng dev server (MutiS: npm run dev trong web, cổng 7880).", invalid: "Chỉ mở được địa chỉ http hoặc https.",
};
