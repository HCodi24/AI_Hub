const assert = require("node:assert/strict");
const path = require("node:path");
module.exports = async function testImportFile(load, web) {
  const { importTextFile, importedFileText, MAX_IMPORT_BYTES } = load(path.join(web, "src/lib/import-file.ts"));
  const file = (name, bytes) => ({ name, size: bytes.length, arrayBuffer: async () => Uint8Array.from(bytes).buffer });
  const textFile = (name, text) => file(name, Buffer.from(text));
  const vietnamese = "Tiếng Việt: đọc file đính kèm";
  assert.deepEqual(await importTextFile(textFile("notes.md", vietnamese)), { name: "notes.md", content: vietnamese });
  assert.equal((await importTextFile(file("unicode.txt", Buffer.concat([Buffer.from([255, 254]), Buffer.from(vietnamese, "utf16le")])))).content, vietnamese);
  for (const name of ["data.csv", "data.json", "script.py", ".env", "README"]) {
    assert.equal((await importTextFile(textFile(name, "hello"))).content, "hello");
  }
  for (const input of [textFile("document.pdf", "%PDF-1.7"), textFile("document.docx", "not text"), textFile("renamed.txt", "%PDF-1.7"), file("binary.bin", [1, 2, 0, 3]), file("bad-utf8.txt", [255, 255]), textFile("empty.txt", " \n\t")]) {
    await assert.rejects(importTextFile(input));
  }
  await assert.rejects(importTextFile({ name: "large.txt", size: MAX_IMPORT_BYTES + 1, arrayBuffer: () => { throw Error("must reject before reading"); } }));
  const imported = [{ name: 'a"\n.md', content: "```js\nconst value = `hello`;\n```" }, { name: "b.txt", content: vietnamese }];
  const prompt = importedFileText(imported);
  assert.ok(prompt.includes(JSON.stringify(imported[0].name)));
  assert.ok(prompt.includes("````\n```js"));
  assert.ok(prompt.includes(vietnamese));
  assert.equal(importedFileText([]), "");
  assert.ok(importedFileText([{ name: "many.txt", content: "` ".repeat(100000) }]).length > 200000);
  console.log("PASS file import: Unicode text/code, size and binary rejection, original filenames and safe Markdown fences");
};
