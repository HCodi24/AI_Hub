const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('../web/node_modules/typescript');
const { chatText } = (() => {
  const module = { exports: {} };
  vm.runInNewContext(ts.transpileModule(fs.readFileSync('web/src/text/chat.ts', 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS },
  }).outputText, { module, exports: module.exports });
  return module.exports;
})();
class ApiError extends Error { constructor(status) { super(); this.status = status; } }
let states = [], cursor = 0, effect, listDir;
const jsx = (type, props) => ({ type, props });
const moduleUnderTest = { exports: {} };
const mocks = {
  react: { useState(initial) {
    const index = cursor++;
    if (!(index in states)) states[index] = initial;
    return [states[index], value => { states[index] = value; }];
  }, useEffect(fn) { effect = fn; } },
  'react/jsx-runtime': { jsx, jsxs: jsx },
  'lucide-react': {},
  '@/components/dialog/dialog': { Dialog: 'dialog' },
  '@/components/button/button': { Button: 'button' },
  '@/apis/config.api': { configApi: { listDir: path => listDir(path) } },
  '@/text/chat': { chatText }, '@/lib/utils': { folderName: path => path },
  '@/lib/http': { ApiError },
  './folder-tree.module.css': { default: {} },
};
vm.runInNewContext(ts.transpileModule(fs.readFileSync('web/src/components/features/chat/folder-tree.tsx', 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
}).outputText, { exports: moduleUnderTest.exports, require: name => mocks[name] });
const props = { folder: 'C:/project' };
function render() { cursor = 0; return moduleUnderTest.exports.FolderTree(props); }
const flush = () => new Promise(resolve => setImmediate(resolve));
async function run() {
  listDir = async () => { throw new ApiError(404); };
  render(); effect(); await flush();
  let view = render();
  assert.match(JSON.stringify(view), /tree|Backend/);
  assert.equal(states[0], null, 'failed requests must never become an empty folder');
  assert.equal(states[1], chatText.treeBackendMissing);
  const alert = view.props.children[2].props.children;
  alert.props.children[2].props.onClick();
  assert.equal(states[2], 1, 'retry starts another request');
  listDir = async () => [];
  render(); effect(); await flush();
  assert.equal(render().props.children[2].props.children, chatText.treeEmpty);
  listDir = async () => [{ name: 'src', dir: true }];
  effect(); await flush();
  assert.equal(render().props.children[2].props.children[0].props.entry.name, 'src');
  listDir = async () => { throw new Error('network'); };
  effect(); await flush();
  assert.equal(states[1], chatText.treeLoadFailed);
  let resolve;
  listDir = () => new Promise(done => { resolve = done; });
  render(); const cleanup = effect(); cleanup();
  resolve([{ name: 'stale', dir: false }]); await flush();
  assert.equal(states[0], null, 'ignore a response after changing folders or closing');
  console.log('PASS folder tree: 404, generic error, retry, empty, populated, stale response');
}
run().catch(error => { console.error(error); process.exitCode = 1; });
