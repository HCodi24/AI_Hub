const assert = require("node:assert/strict");
const path = require("node:path");
// Messages typed during a run wait in line, in order, and can be removed one by one.
module.exports = function testChatQueue(load, web) {
  const { useChatStore } = load(path.join(web, "src/components/features/chat/chat-store.ts"));
  const store = useChatStore;
  store.getState().enqueue({ text: "a", attachments: [] });
  store.getState().enqueue({ text: "b", attachments: [] });
  store.getState().enqueue({ text: "c", attachments: [] });
  store.getState().unqueue(1);
  assert.equal(store.getState().dequeue().text, "a");
  assert.equal(store.getState().dequeue().text, "c");
  assert.equal(store.getState().dequeue(), undefined);
  store.getState().enqueue({ text: "d", attachments: [] });
  store.getState().clearQueue();
  assert.deepEqual(store.getState().queue, []);
  console.log("PASS chat queue: order, remove, empty, clear");
};
