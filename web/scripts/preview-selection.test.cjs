const assert = require("node:assert/strict");
const fs = require("node:fs");
const ts = require("typescript");
const source = fs.readFileSync(require("node:path").join(__dirname, "../src/lib/preview-selection.ts"), "utf8");
const compiled = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS } }).outputText;
const loaded = { exports: {} };
new Function("exports", compiled)(loaded.exports);
const { previewSelectionText, splitPreviewSelectionText } = loaded.exports;
const items = [
  { label: 'main > div[title="test"]', text: 'CSS: display: flex\n```html\n<div>Tiếng Việt</div>\n```' },
  { label: "Vùng 126×59 (1 phần tử)", text: "````\nNested fence\n````" },
];
const encoded = previewSelectionText(items);
assert.deepEqual(splitPreviewSelectionText(`Question\n\n${encoded}`), { text: "Question", selections: items });
assert.deepEqual(splitPreviewSelectionText(encoded), { text: "", selections: items });
assert.deepEqual(splitPreviewSelectionText(encoded.replaceAll("\n", "\r\n")).selections.map(item => item.label), items.map(item => item.label));
for (const text of ["Ordinary message", 'Preview selection "broken":\n```\nincomplete', 'Preview selection "\\q":\n```\ntext\n```']) {
  assert.deepEqual(splitPreviewSelectionText(text), { text, selections: [] });
}
console.log("preview-selection: passed");
