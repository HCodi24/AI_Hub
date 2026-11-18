const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('../web/node_modules/typescript');
const mod = { exports: {} };
vm.runInNewContext(ts.transpileModule(fs.readFileSync('web/src/constants/chat.ts', 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS },
}).outputText, { exports: mod.exports });
const { EFFORTS, effortsFor, effortFor } = mod.exports;
for (const model of ['DeepSeek-V4.1-Flash', 'deepseek-v4.1-flash', 'deepseek-flash']) {
  assert.deepEqual(Array.from(effortsFor(model, 'openai_compat')), ['low', 'high', 'max']);
  for (const value of ['low', 'high', 'max']) assert.equal(effortFor(model, value), value);
  for (const value of ['default', 'medium', 'xhigh']) assert.equal(effortFor(model, value), 'high');
}
assert.equal(effortsFor('gpt-model', 'codex_cli'), EFFORTS);
assert.equal(effortsFor('DeepSeek-V4.1-Flash', 'codex_cli'), EFFORTS);
assert.equal(effortFor('other-model', 'medium'), 'medium');
assert.deepEqual(Array.from(effortsFor('gpt-6.2-sol', 'codex_cli')), ['low', 'medium', 'high']);
for (const value of ['low', 'medium', 'high']) assert.equal(effortFor('gpt-6.2-sol', value, 'codex_cli'), value);
for (const value of ['default', 'xhigh', 'max']) assert.equal(effortFor('gpt-6.2-sol', value, 'codex_cli'), 'medium');
assert.equal(effortsFor('gpt-6.1-sol', 'codex_cli'), EFFORTS);
console.log('PASS model efforts: DeepSeek Low/High/Max, old settings normalized, other models unchanged');
