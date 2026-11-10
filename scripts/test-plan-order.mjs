import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

// Import the browser's pure module without requiring a package.json for the app.
const source = await readFile(new URL("../ui/js/plan-order.js", import.meta.url), "utf8");
const { planRunOrder } = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);
const task = (id, depends_on = [], status = "chờ") => ({ id, depends_on, status });
assert.deepEqual(planRunOrder([task(3, [2]), task(1), task(2, [1])]), [1, 2, 3]);
assert.deepEqual(planRunOrder([task(1, [], "xong"), task(3, [1]), task(2, [1])]), [3, 2]);
assert.deepEqual(planRunOrder([task(1, [], "lỗi"), task(2, [1])]), [1, 2]);
assert.deepEqual(planRunOrder([]), []);
assert.throws(() => planRunOrder([task(1, [2]), task(2, [1])]), /vòng phụ thuộc/u);
assert.throws(() => planRunOrder([task(1, [9])]), /Thiếu/u);
assert.throws(() => planRunOrder([task(1), task(1)]), /trùng/u);
console.log("planRunOrder: 7 checks passed");
