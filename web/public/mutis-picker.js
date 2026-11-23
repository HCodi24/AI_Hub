/* MutiS element picker. Put <script src="…/mutis-picker.js"></script> in a dev page that the MutiS preview panel shows.
 * Idle until the panel (the parent window) sends {type:"mutis-pick-start"}; then a click on an element sends its HTML,
 * selector and main CSS back to that same window only. A press-and-drag instead selects every element inside the dragged
 * region. Esc or the panel button stops it. */
(function () {
  if (window.parent === window || window.__mutisPicker) return;
  var desktopOrigin = typeof mutisPickerParentOrigin === "string" ? mutisPickerParentOrigin : null;
  if (desktopOrigin && (window.parent !== window.top || !/^https?:$/.test(location.protocol) ||
      (location.ancestorOrigins && location.ancestorOrigins[0] !== desktopOrigin))) return;
  window.__mutisPicker = true;
  var parentOrigin = desktopOrigin || (document.currentScript?.dataset.sameOriginOnly === "true" ? location.origin : "*");
  var CSS = ["color", "background-color", "font-family", "font-size", "font-weight", "display", "width", "height", "margin", "padding", "border", "border-radius"];
  var panel = null; // the parent window and its origin: the only place results go
  var box = null;

  function selector(el) {
    var parts = [];
    for (; el && el.nodeType === 1 && parts.length < 5; el = el.parentElement) {
      var part = el.tagName.toLowerCase();
      if (el.id) { parts.unshift(part + "#" + el.id); break; }
      var cls = String(el.getAttribute("class") || "").trim().split(/\s+/).filter(Boolean).slice(0, 2);
      if (cls.length) part += "." + cls.join(".");
      parts.unshift(part);
    }
    return parts.join(" > ");
  }
  function describe(el) {
    var style = getComputedStyle(el), css = {};
    CSS.forEach(function (name) { css[name] = style.getPropertyValue(name); });
    var html = el.outerHTML;
    return {
      url: location.href, selector: selector(el), text: (el.innerText || "").trim().slice(0, 200), css: css,
      html: html.length > 1500 ? html.slice(0, 1500) + "…" : html,
    };
  }
  var down = null, dragging = false, SKIP = /^(SCRIPT|STYLE|NOSCRIPT|META|LINK|HEAD|BR|TITLE)$/;
  /** A modal <dialog> sits in the browser's top layer, above everything else, so the box must live inside it. */
  var boxHost = null;
  function hostFor(el) { return (el instanceof Element && el.closest("dialog[open]")) || document.documentElement; }
  function origin(host) {
    if (host === document.documentElement) return { left: 0, top: 0 };
    var s = getComputedStyle(host);
    if (s.transform === "none" && s.filter === "none" && s.perspective === "none" && !/paint|layout|strict|content/.test(s.contain)) return { left: 0, top: 0 };
    var r = host.getBoundingClientRect();
    return { left: r.left + host.clientLeft, top: r.top + host.clientTop };
  }
  function frame(rect, dashed) {
    var host = boxHost || document.documentElement, o = origin(host);
    if (box.parentNode !== host) host.appendChild(box);
    box.style.cssText = "position:fixed;pointer-events:none;z-index:2147483647;border:2px " + (dashed ? "dashed" : "solid") +
      " #3b82f6;background:rgba(59,130,246,.15);left:" + (rect.left - o.left) + "px;top:" + (rect.top - o.top) + "px;width:" + rect.width + "px;height:" + rect.height + "px";
  }
  function dragRect(event) {
    var left = Math.min(down.x, event.clientX), top = Math.min(down.y, event.clientY);
    return { left: left, top: top, width: Math.abs(event.clientX - down.x), height: Math.abs(event.clientY - down.y) };
  }
  /** Elements mostly inside the region; a container whose parent is also inside is left to that parent's HTML. */
  function inside(region) {
    var right = region.left + region.width, bottom = region.top + region.height, found = new Set();
    document.body.querySelectorAll("*").forEach(function (el) {
      if (SKIP.test(el.tagName) || el === box || getComputedStyle(el).visibility === "hidden") return;
      var r = el.getBoundingClientRect(), area = r.width * r.height;
      if (!area) return;
      var w = Math.min(r.right, right) - Math.max(r.left, region.left), h = Math.min(r.bottom, bottom) - Math.max(r.top, region.top);
      if (w > 0 && h > 0 && (w * h) / area >= 0.5) found.add(el);
    });
    var top = Array.from(found).filter(function (el) {
      for (var p = el.parentElement; p; p = p.parentElement) if (found.has(p)) return false;
      return true;
    });
    // Nothing fits inside: the region lies within one element, so take the smallest one under its centre.
    if (!top.length) { var hit = document.elementFromPoint(region.left + region.width / 2, region.top + region.height / 2); if (hit && hit !== box) top.push(hit); }
    return top;
  }
  function describeRegion(region) {
    var all = inside(region), items = [], size = 0;
    all.slice(0, 20).forEach(function (el) {
      var r = el.getBoundingClientRect(), style = getComputedStyle(el), html = el.outerHTML;
      var item = {
        selector: selector(el), text: (el.innerText || el.getAttribute("alt") || "").trim().slice(0, 120),
        at: { left: Math.round(r.left - region.left), top: Math.round(r.top - region.top), width: Math.round(r.width), height: Math.round(r.height) },
        css: { color: style.color, "background-color": style.backgroundColor, "font-size": style.fontSize, display: style.display },
        html: html.length > 600 ? html.slice(0, 600) + "…" : html,
      };
      size += item.html.length + item.text.length + 200;
      if (size <= 4000 || !items.length) items.push(item);
    });
    return { kind: "region", url: location.href, width: Math.round(region.width), height: Math.round(region.height), total: all.length, items: items };
  }
  function move(event) {
    if (!box) return;
    if (down && !dragging && Math.hypot(event.clientX - down.x, event.clientY - down.y) >= 5) dragging = true;
    if (dragging) return frame(dragRect(event), true);
    var el = event.target;
    if (el instanceof Element) { boxHost = hostFor(el); frame(el.getBoundingClientRect(), false); }
  }
  function press(event) {
    if (event.button !== 0) return;
    event.preventDefault(); event.stopPropagation();
    down = { x: event.clientX, y: event.clientY }; dragging = false; boxHost = hostFor(event.target);
  }
  function release(event) {
    if (!down) return;
    event.preventDefault(); event.stopPropagation();
    var target = panel, wasDragging = dragging, region = wasDragging ? dragRect(event) : null, el = event.target;
    stop();
    // The click that follows mouseup would otherwise reach the page's own handlers.
    var swallow = function (e) { e.preventDefault(); e.stopPropagation(); };
    document.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(function () { document.removeEventListener("click", swallow, true); }, 300);
    target.source.postMessage({ type: "mutis-picked", data: wasDragging ? describeRegion(region) : describe(el) }, target.origin);
  }
  function key(event) { if (event.key === "Escape") { var target = panel; stop(); target.source.postMessage({ type: "mutis-pick-end" }, target.origin); } }
  function start(source, origin) {
    stop();
    panel = { source: source, origin: origin };
    box = document.createElement("div");
    document.documentElement.appendChild(box);
    document.addEventListener("mousemove", move, true);
    document.addEventListener("mousedown", press, true);
    document.addEventListener("mouseup", release, true);
    document.addEventListener("keydown", key, true);
    document.documentElement.style.cursor = "crosshair";
  }
  function stop() {
    document.removeEventListener("mousemove", move, true);
    document.removeEventListener("mousedown", press, true);
    document.removeEventListener("mouseup", release, true);
    document.removeEventListener("keydown", key, true);
    document.documentElement.style.cursor = "";
    if (box) box.remove();
    box = null; panel = null; down = null; dragging = false; boxHost = null;
  }
  window.addEventListener("message", function (event) {
    if (event.source !== window.parent || !event.data) return;
    if (parentOrigin !== "*" && event.origin !== parentOrigin) return;
    if (event.data.type === "mutis-pick-start") start(event.source, event.origin);
    else if (event.data.type === "mutis-pick-stop") stop();
    else if (event.data.type === "mutis-picker-ping") event.source.postMessage({ type: "mutis-picker-ready" }, event.origin);
  });
  // Content-free hello so the panel knows the picker is installed; nothing else is sent unasked.
  window.parent.postMessage({ type: "mutis-picker-ready" }, parentOrigin);
})();
