type Json = Record<string, unknown>;
const isObject = (value: unknown): value is Json => typeof value === "object" && value !== null && !Array.isArray(value);
const reply = (value: unknown) => new Response(JSON.stringify(value ?? {}), { status: 200, headers: { "Content-Type": "application/json" } });

/** What a sign-in/sign-out button would do on the server, for any status object that has `connected`. */
function applyAction(status: Json, action: unknown): Json {
  if (action === "cli" || action === "web") return { ...status, connected: true, pending: false, email: "preview@example.com", origin: action, message: "Đã đăng nhập (xem trước)" };
  if (action === "logout") return { ...status, connected: false, pending: false, email: null, origin: null, plan: null, message: "Đã đăng xuất (xem trước)" };
  if (action === "cancel") return { ...status, pending: false };
  if (action === "sandbox" && "sandbox" in status) return { ...status, sandbox: "ready" };
  return status;
}

/**
 * Preview only: reads go to the real server once and are remembered; writes never leave the browser, they change the
 * remembered copy, so a button shows its effect without touching real data.
 * shortcut: a write changes only the same path (a delete or an add does not change the list at another path);
 * add handlers here when a screen needs them.
 */
export function installPreviewMock() {
  const real = window.fetch.bind(window);
  const remembered = new Map<string, unknown>();
  window.fetch = async (input, init) => {
    const request = input instanceof Request ? input : null;
    const url = new URL(request ? request.url : String(input), window.location.origin);
    if (!url.pathname.startsWith("/api/")) return real(input, init);
    const method = (init?.method ?? request?.method ?? "GET").toUpperCase();
    const key = url.pathname;
    if (method === "GET") {
      if (remembered.has(key)) return reply(remembered.get(key));
      const response = await real(input, init);
      if (response.ok) remembered.set(key, await response.clone().json().catch(() => undefined));
      return response;
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
