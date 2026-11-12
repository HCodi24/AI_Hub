const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('../web/node_modules/typescript');
const { QueryClient } = require('../web/node_modules/@tanstack/react-query');
const client = new QueryClient();
let calls = 0, pending = [];
const mod = { exports: {} };
const mocks = {
  '@tanstack/react-query': { useQueries({ queries }) {
    return queries.map(options => {
      assert.equal(options.enabled, undefined, 'checks must not depend on opening the menu');
      assert.equal(options.staleTime, Infinity);
      assert.equal(options.gcTime, Infinity);
      assert.equal(options.refetchOnWindowFocus, false);
      assert.equal(options.refetchOnReconnect, false);
      pending.push(client.fetchQuery(options));
      return { data: client.getQueryData(options.queryKey) };
    });
  } },
  '@/apis/settings.api': { settingsApi: { testProvider: async id => { calls++; return { ok: id === 'one', status: 'checked' }; } } },
};
vm.runInNewContext(ts.transpileModule(fs.readFileSync('web/src/lib/provider-connections.ts', 'utf8'), {
  compilerOptions: { module: ts.ModuleKind.CommonJS },
}).outputText, { exports: mod.exports, require: name => mocks[name] });
const providers = [{ id: 'one' }, { id: 'two' }];
const render = () => mod.exports.useProviderConnections(providers);
async function flush() { await Promise.all(pending); pending = []; }
async function run() {
  render(); await flush();
  let result = render(); await flush();
  assert.equal(calls, 2);
  assert.equal(result.one.ok, true);
  assert.equal(result.two.ok, false);
  for (let i = 0; i < 5; i++) { render(); await flush(); }
  assert.equal(calls, 2, 'reopening menus and Settings reuses the same results');
  client.setQueryData(['provider-test', 'two'], { ok: true });
  result = render(); await flush();
  assert.equal(result.two.ok, true, 'manual Settings result appears immediately');
  assert.equal(calls, 2);
  await client.invalidateQueries({ queryKey: ['provider-test'] });
  render(); await flush();
  assert.equal(calls, 4, 'configuration changes allow fresh checks');
  client.clear();
  console.log('PASS shared provider status: startup check, cache reuse, Settings update and invalidation');
}
run().catch(error => { client.clear(); console.error(error); process.exitCode = 1; });
