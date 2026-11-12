const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('../web/node_modules/typescript');
const compile = file => ts.transpileModule(fs.readFileSync(file, 'utf8'), { compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX } }).outputText;
const constants = { exports: {} };
vm.runInNewContext(compile('web/src/constants/chat.ts'), { exports: constants.exports });
const role = (provider_id, model, effort) => ({ provider_id, model, effort });
let current = {
  high_roles: { lead: role('codex', 'gpt-6.1-sol', 'medium'), big: role('codex', 'gpt-6.1-sol', 'medium'), small: role('deepseek', 'deepseek-flash', 'max') },
  roles: { lead: role('claude', 'opus', 'high'), big: role('codex', 'gpt-6.1-sol', 'high'), small: role('deepseek', 'deepseek-flash', 'high') },
};
let saved, cached, fail = false, invalidated = false;
const jsx = (type, props) => ({ type, props });
const mocks = {
  react: { useEffect() {}, useRef: value => ({ current: value }), useState: value => [value, () => {}] },
  'react/jsx-runtime': { jsx, jsxs: jsx },
  '@tanstack/react-query': { useQueryClient: () => ({ setQueryData: (_, value) => { cached = value; }, invalidateQueries: () => { invalidated = true; } }) },
  '@/apis/mutis.api': { mutisApi: { get: async () => current, put: async value => { if (fail) throw Error('save failed'); saved = current = value; return value; } } },
  '@/constants/chat': constants.exports,
  '@/text/chat': { chatText: {} },
  './model-menu': { modelLabel: value => value },
  './chat-toolbar.module.css': { default: {} },
};
const mod = { exports: {} };
vm.runInNewContext(compile('web/src/components/features/composer/mutis-efforts.tsx'), { exports: mod.exports, require: name => { assert.ok(mocks[name], name); return mocks[name]; } });
const config = { providers: [{ id: 'codex', kind: 'codex_cli' }, { id: 'deepseek', kind: 'openai_compat' }, { id: 'claude', kind: 'claude_cli' }] };
const render = (level, disabled = false) => mod.exports.MutisEfforts({ mutis: current, level, config, disabled });
(async () => {
  let cards = render('high').props.children[0];
  assert.equal(cards.length, 2, 'High groups shared Codex roles into one slider');
  const deepseekView = cards[1].type(cards[1].props);
  const range = deepseekView.props.children[1].props.children[0];
  assert.equal(range.props.max, 2, 'DeepSeek Flash has three supported effort choices');
  const maxBefore = JSON.stringify(current.roles);
  assert.equal(await cards[0].props.save('xhigh'), true);
  assert.equal(saved.high_roles.lead.effort, 'xhigh');
  assert.equal(saved.high_roles.big.effort, 'xhigh');
  assert.equal(saved.high_roles.small.effort, 'max');
  assert.equal(JSON.stringify(saved.roles), maxBefore, 'High does not change Max');
  assert.equal(cached, saved);
  cards = render('max').props.children[0];
  assert.equal(cards.length, 3, 'Max preserves all distinct configured models');
  await cards[2].props.save('low');
  assert.equal(saved.roles.small.effort, 'low');
  assert.equal(saved.high_roles.small.effort, 'max');
  saved = null;
  await render('high', true).props.children[0][0].props.save('low');
  assert.equal(saved, null, 'Streaming prevents configuration writes');
  fail = true;
  assert.equal(await render('high').props.children[0][0].props.save('low'), false);
  assert.equal(invalidated, true, 'Failed writes refresh server configuration');
  console.log('PASS MutiS effort sliders: grouping, supported choices, shared roles, profile isolation, persistence, disabled state, save failure');
})().catch(error => { console.error(error); process.exitCode = 1; });
