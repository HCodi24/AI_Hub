---
name: demo
description: Xem riêng một thành phần giao diện của dự án trong bảng xem trước MutiS bằng dữ liệu mẫu.
---

# /demo

Với `/demo <nội dung muốn xem>`, chỉ hiển thị thành phần giao diện được yêu cầu trong bảng xem trước MutiS. Dùng mã nguồn của dự án trong thư mục chat, tách thành phần khỏi phần còn lại của ứng dụng và dùng dữ liệu mẫu. Áp dụng với mọi framework.

1. Tìm thành phần trong mã nguồn. Nếu thiếu đối số, không tìm thấy hoặc có nhiều cách hiểu, dùng thẻ câu hỏi của ứng dụng (`mutis-questions`) để hỏi người dùng muốn xem gì. Đừng đoán.
2. Nếu dự án đã có màn hình được Git theo dõi hiển thị đúng thành phần, dùng URL của màn hình đó mà không sửa tệp.
3. **Tuyệt đối không tạo hoặc sửa tệp do Git theo dõi.** Không sửa `.gitignore`, `package.json`, tệp cấu hình build hoặc framework, hay tệp khóa phụ thuộc. Chỉ ghi vào thư mục demo riêng trong dự án. Luôn đặt tên thư mục demo đúng là `__mutis-demo` hoặc `.mutis-demo` theo hướng dẫn bên dưới vì MutiS dựa vào tên thư mục để tìm và xoá tệp demo. Đặt `<slug>` theo tên thành phần ở dạng kebab-case. Dùng một tệp hoặc một cặp tệp cho mỗi thành phần. Cập nhật demo sẵn có thay vì tạo bản trùng.
   - Next.js Pages Router dùng `<pages dir>/__mutis-demo/<slug>` với phần mở rộng theo `pageExtensions`. Với MutiS, dùng `web/src/pages/__mutis-demo/<slug>.page.tsx`.
   - Next.js App Router dùng `<app dir>/__mutis-demo/<slug>/page.tsx`.
   - Vite dùng `.mutis-demo/<slug>.html` và một tệp điểm vào trong `.mutis-demo/` tại thư mục gốc của Vite.
   - Với thiết lập khác, chọn vị trí tương đương mà máy chủ phát triển phục vụ được. Nếu chưa xác định được vị trí, hỏi người dùng.
4. Nếu dự án dùng Git, kiểm tra trước khi ghi để chắc chắn tệp đích chưa được theo dõi. Chỉ khi thiếu mục loại trừ, nối một dòng chứa đường dẫn thư mục demo tính từ gốc kho Git vào cuối `.git/info/exclude`. Tạo tệp này nếu chưa có. Sau khi ghi, chạy `git status --porcelain` và xác nhận không có tệp demo nào xuất hiện. Nếu có, sửa mục loại trừ rồi kiểm tra lại. Không đưa tệp demo vào chỉ mục Git.
5. Nhập thành phần thật và hiển thị 2–4 mẫu dữ liệu thực tế, mỗi mẫu có chú thích ngắn. Chọn các trạng thái phù hợp như bình thường, trống, văn bản dài, lỗi hoặc bị vô hiệu hóa. Hàm gọi lại chỉ hiển thị giá trị ngay trên trang, chẳng hạn `Đã chọn: ...`. Không gửi yêu cầu ghi qua mạng, dùng dữ liệu thật của người dùng hoặc thêm phụ thuộc. Khi cần giao diện chủ đề, kho trạng thái hoặc query client, dùng lại bản giả lập và provider có sẵn của dự án.
6. Dùng địa chỉ máy chủ phát triển của dự án. Đọc script trong `package.json` và cấu hình framework để xác định cổng và đường dẫn gốc. Giao diện phát triển MutiS dùng `http://localhost:7880/app/...`. Bảng xem trước có thể tự khởi động máy chủ phát triển.
7. Trả lời bằng tiếng Việt. Nếu đã tạo hoặc cập nhật demo, liệt kê các tệp trong thư mục demo, xác nhận đã loại trừ khỏi Git nếu dự án dùng Git, và cho người dùng biết họ có thể xoá tất cả tệp demo bằng nút «Xoá demo» trong bảng xem trước MutiS (hoặc tự xoá thư mục demo). Nếu dùng màn hình sẵn có, nêu rõ màn hình đã dùng. Kết thúc bằng đúng một dòng `preview: <url>`, thay `<url>` bằng URL đầy đủ và để dòng đó ở dạng văn bản thuần.
