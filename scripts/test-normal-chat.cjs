const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('../web/node_modules/typescript');
let sent, planned = false, shownBeforeResolve = false;
const state = {
  id: '00000000-0000-4000-8000-000000000001', messages: [], streaming: false,
  settings: { provider_id: 'p', model: 'model', folder: '', permission: 'write' },
  begin(text) { state.messages = [{ role: 'user', content: text }, { role: 'assistant', content: '' }]; },
  event() {}, finish(error) { assert.equal(error, undefined); },
};
const mocks = {
  react: { useRef: value => ({ current: value }) },
  '@tanstack/react-query': { useQueryClient: () => ({ invalidateQueries: async () => {} }), useQuery: () => ({ data: { providers: [{ id: 'p' }] } }) },
  '@/lib/agent-source': { fileToolsFor: () => 'write' },
  '@/apis/chat.api': { chatApi: { send: async request => { sent = request; } } },
  '@/apis/config.api': {},
  './chat-store': { useChatStore: { getState: () => state, setState: patch => Object.assign(state, patch) } },
  '@/text/chat': { chatText: {} },
  '@/apis/mutis.api': { mutisApi: {
    get: async () => ({ enabled: true, auto_plan: true, roles: { lead: { provider_id: 'p', model: 'model' } } }),
    resolve: async () => { shownBeforeResolve = state.messages.length === 2; return { lead: { used_provider: { id: 'p' } } }; },
  } },
  './card-actions': { startCard: async () => { planned = true; } },
  './card-store': { useCardStore: { getState: () => ({ running: false }) } },
  '@/lib/mutis-level': { rolesFor: () => undefined },
  '@/components/features/memory/memory-events': {}, '@/components/toast/toast': {},
};
const mod = { exports: {} };
vm.runInNewContext(ts.transpileModule(fs.readFileSync('web/src/components/features/chat/use-chat.ts', 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText, { exports: mod.exports, require: name => mocks[name], AbortController,
  localStorage: { getItem: () => 'true' } });
async function run() {
  const chat = mod.exports.useChat();
  assert.equal(await chat.send('Hello'), true);
  assert.equal(sent.folder, '');
  assert.equal(sent.file_tools, 'none');
  assert.equal(planned, false, 'ordinary chats skip debate and project auto-planning');
  state.settings.provider_id = 'mutis';
  assert.equal(await chat.send('Hello again'), true);
  assert.equal(sent.provider_id, 'p');
  assert.equal(shownBeforeResolve, true, 'the message is on screen before the MutiS source is resolved');
  assert.equal(sent.folder, '');
  assert.equal(planned, false);
  state.settings.folder = 'C:/project';
  await chat.send('Project request');
  assert.equal(planned, true, 'project chats retain debate routing');
  planned = false;
  await chat.send('Direct request', true);
  assert.equal(sent.folder, 'C:/project');
  assert.equal(sent.file_tools, 'write');
  console.log('PASS ordinary chat: empty project, no file tools, direct routing, MutiS lead and project behavior');
}
run().catch(error => { console.error(error); process.exitCode = 1; });
