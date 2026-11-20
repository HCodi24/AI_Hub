/* MutiS element picker. Put <script src="…/mutis-picker.js"></script> in a dev page that the MutiS preview panel shows.
 * Idle until the panel (the parent window) sends {type:"mutis-pick-start"}; then a click on an element sends its HTML,
 * selector and main CSS back to that same window only. Esc or the panel button stops it. */
(function () {
  if (window.parent === window || window.__mutisPicker) return;
  window.__mutisPicker = true;
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
  function move(event) {
    var el = event.target;
    if (!box || !(el instanceof Element)) return;
    var rect = el.getBoundingClientRect();
    box.style.cssText = "position:fixed;pointer-events:none;z-index:2147483647;border:2px solid #3b82f6;background:rgba(59,130,246,.15);left:" +
      rect.left + "px;top:" + rect.top + "px;width:" + rect.width + "px;height:" + rect.height + "px";
  }
  function click(event) {
    event.preventDefault(); event.stopPropagation();
    var target = panel;
    stop();
    target.source.postMessage({ type: "mutis-picked", data: describe(event.target) }, target.origin);
  }
  function key(event) { if (event.key === "Escape") { var target = panel; stop(); target.source.postMessage({ type: "mutis-pick-end" }, target.origin); } }
  function start(source, origin) {
    stop();
    panel = { source: source, origin: origin };
    box = document.createElement("div");
    document.documentElement.appendChild(box);
    document.addEventListener("mousemove", move, true);
    document.addEventListener("click", click, true);
    document.addEventListener("keydown", key, true);
    document.documentElement.style.cursor = "crosshair";
  }
  function stop() {
    document.removeEventListener("mousemove", move, true);
    document.removeEventListener("click", click, true);
    document.removeEventListener("keydown", key, true);
    document.documentElement.style.cursor = "";
    if (box) box.remove();
    box = null; panel = null;
  }
  window.addEventListener("message", function (event) {
    if (event.source !== window.parent || !event.data) return;
    if (event.data.type === "mutis-pick-start") start(event.source, event.origin);
    else if (event.data.type === "mutis-pick-stop") stop();
  });
  // Content-free hello so the panel knows the picker is installed; nothing else is sent unasked.
  window.parent.postMessage({ type: "mutis-picker-ready" }, "*");
})();
