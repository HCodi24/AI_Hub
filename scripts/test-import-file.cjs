const assert = require("node:assert/strict");
const path = require("node:path");
module.exports = async function testImportFile(load, web) {
  const { droppedFiles, fileReadError, importDirectory, importTextFile, importedFileText, splitImportedFileText, MAX_IMPORT_BYTES, MAX_FOLDER_BYTES, MAX_FOLDER_FILES } = load(path.join(web, "src/lib/import-file.ts"));
  const text = load(path.join(web, "src/text/chat.ts")).chatText;
  const file = (name, bytes) => ({ name, size: bytes.length, arrayBuffer: async () => Uint8Array.from(bytes).buffer });
  const textFile = (name, text) => file(name, Buffer.from(text));
  const vietnamese = "Tiếng Việt: đọc file đính kèm";
  const dropped = textFile("notes.md", vietnamese);
  const directory = { kind: "file", webkitGetAsEntry: () => ({ isDirectory: true, name: "pixel-cloud" }), getAsFile: () => { throw Error("must not read a directory as a file"); } };
  const item = { kind: "file", webkitGetAsEntry: () => ({ isDirectory: false }), getAsFile: () => dropped };
  const roots = droppedFiles({ items: [directory, item, { kind: "string" }], files: [] });
  assert.equal(roots[0].isDirectory, true);
  assert.equal(roots[0].name, "pixel-cloud");
  assert.equal(roots[1], dropped);
  assert.deepEqual(droppedFiles({ items: [], files: [dropped] }), [dropped]);
  assert.deepEqual(droppedFiles({ items: [{ kind: "file", getAsFile: () => dropped }], files: [] }), [dropped]);
  assert.deepEqual(droppedFiles({ items: [{ kind: "file", getAsFile: () => null }], files: [] }), []);
  const fileEntry = (name, content) => ({ name, isDirectory: false, file: resolve => resolve(textFile(name, content)) });
  const folderEntry = (name, batches) => ({ name, isDirectory: true, createReader() {
    let page = 0;
    return { readEntries: resolve => resolve(batches[page++] || []) };
  } });
  const ignored = { name: "node_modules", isDirectory: true, createReader() { throw Error("must not scan dependencies"); } };
  const folder = folderEntry("pixel-cloud", [
    [fileEntry("README.md", vietnamese), folderEntry("src", [[fileEntry("index.ts", "const x = 1;")]])],
    [fileEntry("index.ts", "root file"), ignored, fileEntry("manual.pdf", "%PDF-1.7"), fileEntry("large.txt", "a".repeat(MAX_IMPORT_BYTES + 1))],
  ]);
  const importedFolder = await importDirectory(folder);
  assert.equal(importedFolder.name, "pixel-cloud/ (3 file)");
  assert.equal(importedFolder.skipped, 3);
  assert.equal(importedFolder.limited, false);
  for (const name of ["pixel-cloud/README.md", "pixel-cloud/src/index.ts", "pixel-cloud/index.ts"]) assert.ok(importedFolder.content.includes(name));
  assert.ok(importedFolder.content.includes(vietnamese));
  assert.ok(!importedFolder.content.includes("%PDF"));
  const many = await importDirectory(folderEntry("many", [Array.from({ length: MAX_FOLDER_FILES }, (_, i) => fileEntry(`${i}.txt`, "ok")), [fileEntry("extra.txt", "extra")]]));
  assert.equal(many.limited, true);
  assert.ok(many.name.includes(`(${MAX_FOLDER_FILES} file)`));
  assert.ok(!many.content.includes("extra.txt"));
  const large = await importDirectory(folderEntry("large", [Array.from({ length: 6 }, (_, i) => fileEntry(`${i}.txt`, "a".repeat(MAX_IMPORT_BYTES)))]));
  assert.equal(large.limited, true);
  assert.ok(large.name.includes(`(${Math.floor(MAX_FOLDER_BYTES / MAX_IMPORT_BYTES)} file)`));
  await assert.rejects(importDirectory(folderEntry("empty", [])), { message: text.emptyFolder });
  await assert.rejects(importDirectory(folderEntry("binary", [[fileEntry("doc.pdf", "%PDF-1.7")]])), { message: text.emptyFolder });
  let deep = folderEntry("leaf", [[fileEntry("note.txt", "deep content")]]);
  for (let i = 0; i < 34; i++) deep = folderEntry(`level${i}`, [[deep]]);
  await assert.rejects(importDirectory(deep), { message: text.folderScanLimit });
  const crowded = folderEntry("crowded", [Array.from({ length: 2001 }, () => ignored)]);
  await assert.rejects(importDirectory(crowded), { message: text.folderScanLimit });
  const denied = { name: "denied", isDirectory: true, createReader: () => ({ readEntries: (_resolve, reject) => reject(new DOMException("denied", "NotReadableError")) }) };
  await assert.rejects(importDirectory(denied), { name: "NotReadableError" });
  assert.equal(await importDirectory(folder, () => true), null);
  let changedChat = false;
  const switchingFile = { name: "switch.txt", isDirectory: false, file(resolve) { changedChat = true; resolve(textFile("switch.txt", "content")); } };
  assert.equal(await importDirectory(folderEntry("switch", [[switchingFile]]), () => changedChat), null);
  for (const name of ["NotFoundError", "NotReadableError", "SecurityError"]) {
    assert.equal(fileReadError(new DOMException("A requested file or directory could not be found", name)), text.fileUnavailable);
  }
  assert.equal(fileReadError(new Error("upload failed")), "upload failed");
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
  assert.deepEqual(splitImportedFileText(`Please read\n\n${prompt}`), { text: "Please read", files: imported });
  assert.deepEqual(splitImportedFileText(prompt), { text: "", files: imported });
  const folderAttachment = [{ name: importedFolder.name, content: importedFolder.content }];
  assert.deepEqual(splitImportedFileText(importedFileText(folderAttachment)), { text: "", files: folderAttachment });
  const crlf = importedFileText([{ name: "report.json", content: '{"state":"failed"}' }]).replace(/\n/g, "\r\n");
  assert.deepEqual(splitImportedFileText(crlf).files, [{ name: "report.json", content: '{"state":"failed"}' }]);
  for (const content of ["Plain message", "```json\n{\"state\":\"failed\"}\n```", prompt.slice(0, -1), `${prompt}\n\nMore user text`, 'Attached file "bad\\q":\n```\ntext\n```']) {
    assert.deepEqual(splitImportedFileText(content), { text: content, files: [] });
  }
  const report = [{ name: "pixel-cloud/ (1 file)", content: importedFileText([{ name: "report.json", content: JSON.stringify({ frames: Array.from({ length: 2000 }, (_, index) => ({ index, file: `${index}.png` })) }, null, 2) }]) }];
  assert.deepEqual(splitImportedFileText(importedFileText(report)), { text: "", files: report });
  assert.ok(importedFileText([{ name: "many.txt", content: "` ".repeat(100000) }]).length > 200000);
  console.log("PASS file import: recursive/paginated folder drops, relative paths, exclusions, size/count limits, cancellation, unreadable files, Unicode text/code and safe Markdown fences");
};
