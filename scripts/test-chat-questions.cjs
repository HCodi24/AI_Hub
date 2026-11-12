const assert = require("node:assert/strict");
const path = require("node:path");

module.exports = function testChatQuestions(load, web) {
  const { parseChatQuestions, formatQuestionAnswers } = load(path.join(web, "src/lib/chat-questions.ts"));
  const questions = [
    { id: "scope", question: "Nguồn hướng dẫn nào?", options: [
      { label: "Chỉ MutiS", description: "Tách cấu hình cá nhân", recommended: true },
      { label: "Thêm dự án" },
    ] },
    { id: "notes", question: "Yêu cầu thêm?", options: [] },
  ];
  const block = (value) => "```mutis-questions\n" + JSON.stringify(value) + "\n```";
  const source = "Chọn trước khi tiếp tục.\n" + block({ questions });
  const parsed = parseChatQuestions(source);
  assert.deepEqual(parsed.questions, questions);
  assert.equal(parsed.text.trim(), "Chọn trước khi tiếp tục.");
  assert.deepEqual(parseChatQuestions(source.replace(/\n/g, "\r\n")).questions, questions);
  const memory = '<memory>{"open":["Pending decision"]}</memory>';
  assert.ok(parseChatQuestions(source + "\n" + memory).text.includes(memory));

  for (const invalid of [
    "No question.",
    "```mutis-questions\n{broken}\n```",
    block({ questions: [] }),
    block({ questions: [...questions, questions[0]] }),
    block({ questions: [{ ...questions[0], options: Array(6).fill({ label: "x" }) }] }),
    block({ questions: [{ id: "x", question: "", options: [] }] }),
    block({ questions }) + "\n" + block({ questions }),
    "<memory>" + block({ questions }) + "</memory>",
    "````markdown\n" + block({ questions }) + "\n````",
  ]) {
    const result = parseChatQuestions(invalid);
    assert.deepEqual(result.questions, [], invalid);
    assert.equal(result.text, invalid, "Invalid/example content must not disappear");
  }
  const answer = formatQuestionAnswers(questions, { scope: "Chỉ MutiS", notes: "Giữ đăng nhập web" });
  for (const text of ["Nguồn hướng dẫn nào?", "Chỉ MutiS", "Yêu cầu thêm?", "Giữ đăng nhập web"]) {
    assert.ok(answer.includes(text));
  }
  const { ChatRecordSchema } = load(path.join(web, "src/schema/chat.schema.ts"));
  const record = ChatRecordSchema.parse({ id: "8e4e00a4-60ef-4a76-b49c-86d2828ec998", title: "Question", provider_id: "p", model: "m", folder: "", updated_at: 1,
    messages: [{ role: "assistant", content: source }, { role: "user", content: answer }] });
  const restored = ChatRecordSchema.parse(JSON.parse(JSON.stringify(record)));
  assert.deepEqual(parseChatQuestions(restored.messages[0].content).questions, questions);
  assert.equal(restored.messages[1].content, answer);
  const { chatPetState } = load(path.join(web, "src/lib/chat-pet-state.ts"));
  assert.equal(chatPetState(restored.messages[0], false, true), "waiting");
  assert.equal(chatPetState(restored.messages[0], false, false), "idle");
  // Render the actual form to verify no default answer and no active controls on old cards.
  const fs = require("node:fs");
  const vm = require("node:vm");
  const { createRequire } = require("node:module");
  const fromWeb = createRequire(path.join(web, "package.json"));
  const React = fromWeb("react");
  const { renderToStaticMarkup } = fromWeb("react-dom/server");
  const ts = fromWeb("typescript");
  const component = { exports: {} };
  const compiled = ts.transpileModule(fs.readFileSync(path.join(web, "src/components/features/chat/chat-questions.tsx"), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  const resolve = id => {
    if (id.endsWith(".module.css")) return { default: new Proxy({}, { get: (_, key) => key }) };
    if (id === "@/components/button/button") return { Button: ({ variant, ...props }) => React.createElement("button", props) };
    if (id === "@/components/toast/toast") return { notify() {} };
    if (id.startsWith("@/")) return load(path.join(web, "src", id.slice(2) + ".ts"));
    if (id.startsWith("./")) return load(path.join(web, "src/components/features/chat", id.slice(2) + ".ts"));
    return fromWeb(id);
  };
  vm.runInThisContext(`(function(require,module,exports){${compiled}\n})`)(resolve, component, component.exports);
  const props = { questions, send: async () => true, active: true, chatId: record.id };
  const html = renderToStaticMarkup(React.createElement(component.exports.ChatQuestions, props));
  assert.equal((html.match(/type="radio"/g) || []).length, 3);
  assert.ok(!html.includes(" checked="), "Recommendations must not select an answer");
  assert.match(html, /type="submit"[^>]*disabled/);
  assert.ok(html.includes("textarea"), "Free-text-only questions must have an answer input");
  const historical = renderToStaticMarkup(React.createElement(component.exports.ChatQuestions, { ...props, active: false }));
  assert.ok(!historical.includes('type="submit"'));
  assert.ok(!historical.includes("textarea"));
  assert.match(historical, /<fieldset[^>]*disabled/);
  console.log("PASS chat questions: valid/invalid cards, CRLF, memory, examples, answers and history roundtrip");
};
