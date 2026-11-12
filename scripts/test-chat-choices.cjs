const assert = require("node:assert/strict");
const path = require("node:path");
module.exports = function testChatChoices(load, web) {
  const { chatChoices } = load(path.join(web, "src/lib/chat-choices.ts"));
  const screenshot = "Bạn có muốn tôi **build `.codegraph` bằng MCP rồi đọc và tóm tắt dự án** không?\n\n- **Có:** Tôi sẽ build chỉ mục, đọc rồi tóm tắt cấu trúc.\n- **Không:** Tôi sẽ đọc bằng công cụ thông thường.";
  for (const content of [screenshot, "Bạn có muốn tôi sửa tiếp không?", "Có cần khởi động lại không?", "Bạn đồng ý không?", "Chọn phương án nào?\n1. **Có:** tiếp tục\n2. **Không:** dừng"]) {
    assert.deepEqual(chatChoices(content), ["Có", "Không"], content);
  }
  for (const content of ["Would you like me to continue?", "Proceed?\n- Yes: continue\n- No: stop"]) {
    assert.deepEqual(chatChoices(content), ["Yes", "No"], content);
  }
  for (const content of ["Đã sửa xong.", "Bạn muốn chọn file nào?", "Bạn có muốn sửa không? Bạn có muốn build không?", "> Bạn có muốn sửa không?", "```text\nBạn có muốn sửa không?\n```", "~~~text\nBạn có muốn sửa không?\n~~~", "`Bạn có muốn sửa không?`", "<memory>Would you like to continue?</memory>", "<memory>Would you like to continue?", "Có: tiếp tục\nKhông: dừng"]) {
    assert.deepEqual(chatChoices(content), [], content);
  }
  assert.deepEqual(chatChoices(screenshot + '<memory>{"open":["Build?"]}</memory>'), ["Có", "Không"]);
  assert.deepEqual(chatChoices(screenshot + '\n```js\nconst question = "?";\n```'), ["Có", "Không"]);
  console.log("PASS quick replies: screenshot, Vietnamese/English, ambiguous questions, code/quotes and hidden memory");
};
