const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createRequire } = require('node:module');
const webRequire = createRequire(path.resolve('web/package.json'));
const ts = webRequire('typescript');
const saved = new Map();
const storage = { getItem: key => saved.get(key) ?? null, setItem: (key, value) => saved.set(key, value), removeItem: key => saved.delete(key) };
function load(file, mocks = {}) {
  const mod = { exports: {} };
  const code = ts.transpileModule(fs.readFileSync(file, 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  vm.runInNewContext(code, { exports: mod.exports, require: name => mocks[name] ?? webRequire(name),
    localStorage: storage, sessionStorage: storage, crypto: require('node:crypto').webcrypto });
  return mod.exports;
}
const schema = load('web/src/schema/chat.schema.ts');
function store() { return load('web/src/components/features/chat/chat-store.ts', { '@/schema/chat.schema': schema }).useChatStore; }
const state = store();
state.getState().initialize('C:/project', 'codex', 'model');
state.getState().configure({ permission: 'read' });
assert.equal(store().getState().settings.permission, 'write'); // Initialization restores the saved draft.
const reloaded = store();
reloaded.getState().initialize('C:/project', 'codex', 'model');
assert.equal(reloaded.getState().settings.permission, 'read');
const SourceMenu = () => null;
const empty = () => null;
const hooks = { useEffect() {}, useRef: () => ({ current: null }), useState: value => [value, () => {}] };
const toolbar = load('web/src/components/features/composer/chat-toolbar.tsx', {
  react: hooks, '@/components/features/chat/chat-store': { useChatStore: selector => selector(state.getState()) },
  'zustand/react/shallow': { useShallow: selector => selector },
  '@tanstack/react-query': { useQuery: () => ({ data: undefined }) },
  '@/apis/mutis.api': { mutisApi: {} }, '@/schema/config.schema': {},
  '@/text/chat': { chatText: new Proxy({}, { get: (_, key) => key }) },
  '@/constants/chat': { effortsFor: () => ['medium'], effortFor: () => 'medium' },
  '@/lib/mutis-level': { levelFor: () => 'high', rolesFor: () => null },
  '@/lib/agent-source': { isAgentSource: p => Boolean(p?.agent_capable) },
  '@/components/icon-button/icon-button': { IconButton: empty },
  './chat-toolbar.module.css': { default: {} }, './cli-limits': { CliLimits: empty },
  './source-menu': { SourceMenu }, './model-menu': { ModelMenu: empty, modelLabel: x => x },
  './permission-menu': { PermissionMenu: empty },
  './chat-permission': { ChatPermission: empty },
});
const config = { default_folder: 'C:/project', providers: [
  { id: 'codex', kind: 'codex_cli', models: ['model'] },
  { id: 'claude', kind: 'claude_cli', models: ['model'] },
  { id: 'api', kind: 'openai_compat', models: ['model'] },
] };
function find(element, type) {
  if (!element || typeof element !== 'object') return;
  if (element.type === type) return element;
  for (const child of [element.props?.children].flat()) {
    const match = find(child, type); if (match) return match;
  }
}
for (const permission of ['read', 'write', 'full']) {
  state.getState().configure({ provider_id: 'codex', permission });
  find(toolbar.ChatToolbar({ config }), SourceMenu).props.onPick('claude');
  assert.equal(state.getState().settings.permission, permission);
  find(toolbar.ChatToolbar({ config }), SourceMenu).props.onPick('codex');
  assert.equal(state.getState().settings.permission, permission);
}
find(toolbar.ChatToolbar({ config }), SourceMenu).props.onPick('api');
assert.equal(state.getState().settings.permission, 'write');
// A chat keeps its pending permission choice when reopened, separately from other chats.
const chat = { id: 'chat-one', provider_id: 'claude', model: 'model', folder: 'C:/project',
  permission: 'write', effort: 'medium', file_tools: 'none', dev_chat: false, messages: [] };
state.getState().load(chat);
state.getState().configure({ permission: 'read' });
state.getState().load({ ...chat, id: 'chat-two' });
assert.equal(state.getState().settings.permission, 'write');
state.getState().load(chat);
assert.equal(state.getState().settings.permission, 'read');
saved.set('ai-hub-chat-selection:chat-one', JSON.stringify({ provider_id: 'claude', model: 'model', permission: 'invalid' }));
state.getState().load(chat);
assert.equal(state.getState().settings.permission, 'write');
console.log('PASS shared CLI permission: source switches, draft reload, per-chat restore, invalid values and unsupported sources');
