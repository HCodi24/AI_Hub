type Json = Record<string, unknown>;
const isObject = (value: unknown): value is Json => typeof value === "object" && value !== null && !Array.isArray(value);
const reply = (value: unknown) => new Response(JSON.stringify(value ?? {}), { status: 200, headers: { "Content-Type": "application/json" } });

export function installReadOnlyPreview() {
  const real = window.fetch.bind(window);
  window.fetch = (input, init) => {
    const request = input instanceof Request ? input : null;
    const url = new URL(request?.url ?? String(input), window.location.origin);
    const method = (init?.method ?? request?.method ?? "GET").toUpperCase();
    // Provider status uses POST to run the connection probe. Account actions have separate routes.
    if (!url.pathname.startsWith("/api/") || method === "GET" || method === "HEAD"
      || (method === "POST" && /^\/api\/providers\/[^/]+\/test\/?$/.test(url.pathname))) {
      return real(input, init);
    }
    return Promise.resolve(Response.json({ error: "Bản xem trước chỉ cho phép xem dữ liệu" }, { status: 405 }));
  };
}

/** What a sign-in/sign-out button would do on the server, for any status object that has `connected`. */
function applyAction(status: Json, action: unknown): Json {
  if (action === "cli" || action === "web") return { ...status, connected: true, pending: false, email: "preview@example.com", origin: action, message: "Đã đăng nhập (xem trước)" };
  if (action === "logout") return { ...status, connected: false, pending: false, email: null, origin: null, plan: null, message: "Đã đăng xuất (xem trước)" };
  if (action === "cancel") return { ...status, pending: false };
  if (action === "sandbox" && "sandbox" in status) return { ...status, sandbox: "ready" };
  return status;
}

type Mock = "empty" | "many" | "error" | null;
const CACHE = "preview-mock:";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-/i;

/** A copy for the "many" list: unique id/name/title so keys and labels do not repeat. */
function relabel(item: unknown, n: number): unknown {
  if (!isObject(item)) return item;
  const copy: Json = { ...item };
  for (const key of ["id", "name", "title"]) {
    const value = copy[key];
    if (typeof value === "string") copy[key] = key === "id" && UUID.test(value) ? crypto.randomUUID() : `${value} ${n + 1}`;
  }
  return copy;
}

/** `empty` turns every list into []; `many` pads lists up to 20 items. shortcut: lists nested deeper than 2 levels stay as they are, so the count cannot explode. */
function reshape(value: unknown, mock: Mock, depth = 0): unknown {
  if (Array.isArray(value)) {
    if (mock === "empty") return [];
    const items = value.map(item => reshape(item, mock, depth + 1));
    if (mock !== "many" || !items.length || depth > 1) return items;
    return Array.from({ length: Math.max(items.length, 20) }, (_, i) => (i < items.length ? items[i] : relabel(items[i % items.length], i)));
  }
  if (isObject(value)) return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, reshape(item, mock, depth)]));
  return value;
}

const saved = (key: string): unknown => { try { const text = localStorage.getItem(CACHE + key); return text ? JSON.parse(text) : undefined; } catch { return undefined; } };
const save = (key: string, value: unknown) => { try { localStorage.setItem(CACHE + key, JSON.stringify(value)); } catch { /* storage full or blocked: preview still works online */ } };

/**
 * Preview only: reads go to the real server once and are remembered; writes never leave the browser, they change the
 * remembered copy, so a button shows its effect without touching real data.
 * `?mock=empty|many|error` reshapes what reads return. Every real read is also kept in localStorage, so with the
 * server off a screen opens from the last copy (a path never read while online stays unavailable).
 * shortcut: a write changes only the same path (a delete or an add does not change the list at another path);
 * add handlers here when a screen needs them.
 */
export function installPreviewMock() {
  const real = window.fetch.bind(window);
  const remembered = new Map<string, unknown>();
  const param = new URLSearchParams(window.location.search).get("mock");
  const mock: Mock = param === "empty" || param === "many" || param === "error" ? param : null;
  window.fetch = async (input, init) => {
    const request = input instanceof Request ? input : null;
    const url = new URL(request ? request.url : String(input), window.location.origin);
    if (!url.pathname.startsWith("/api/")) return real(input, init);
    const method = (init?.method ?? request?.method ?? "GET").toUpperCase();
    const key = url.pathname;
    if (method === "GET") {
      if (mock === "error") return new Response(JSON.stringify({ error: "Lỗi giả lập (xem trước)" }), { status: 500, headers: { "Content-Type": "application/json" } });
      if (remembered.has(key)) return reply(remembered.get(key));
      // A slow server must not freeze the screen: after 8 s the last saved copy is used.
      const response = await Promise.race([real(input, init).catch(() => null), new Promise<null>(done => setTimeout(done, 8000, null))]);
      const body = response?.ok ? await response.clone().json().catch(() => undefined) : undefined;
      if (body !== undefined) save(key, body);
      const data = body ?? saved(key);
      if (data === undefined) return response ?? new Response(JSON.stringify({ error: "Máy chủ đang tắt" }), { status: 503, headers: { "Content-Type": "application/json" } });
      remembered.set(key, reshape(data, mock));
      return reply(remembered.get(key));
    }
    const raw = typeof init?.body === "string" ? init.body : request ? await request.clone().text() : "";
    let body: unknown;
    try { body = raw ? JSON.parse(raw) : undefined; } catch { body = undefined; }
    const current = remembered.get(key);
    let next: unknown = current;
    if (method === "POST" && isObject(current) && isObject(body)) next = applyAction(current, body.action);
    else if ((method === "PUT" || method === "PATCH") && body !== undefined) next = isObject(current) && isObject(body) ? { ...current, ...body } : body;
    if (next !== undefined) remembered.set(key, next);
    return reply(next ?? body);
  };
}
