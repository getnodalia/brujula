// Zap: pick an element of the page to hide it. Runs in an isolated world of its own, so the page
// cannot see it or call the handler; the browser decides the site and checks the selector.
(function () {
  if (window.__brujulaZap) {
    window.__brujulaZap();
  }
  const outline = document.createElement('div');
  outline.style.cssText =
    'position:fixed;z-index:2147483647;pointer-events:none;border:2px solid #7aa2ff;' +
    'background:rgba(122,162,255,.18);border-radius:4px;transition:all .05s;display:none';
  const note = document.createElement('div');
  note.textContent = 'Haz clic en lo que quieras ocultar · Esc para cancelar';
  note.style.cssText =
    'position:fixed;z-index:2147483647;pointer-events:none;top:12px;left:50%;' +
    'transform:translateX(-50%);padding:8px 14px;border-radius:10px;background:#1c1c22;' +
    'color:#fff;font:13px system-ui,sans-serif;box-shadow:0 6px 20px rgba(0,0,0,.35)';
  document.documentElement.append(outline, note);
  let target = null;

  const usable = (el) =>
    el && el.nodeType === 1 && el !== document.documentElement && el !== document.body &&
    el !== outline && el !== note;

  const word = /^[A-Za-z_][\w-]*$/;
  const path = (start) => {
    const parts = [];
    let el = start;
    while (el && el.nodeType === 1 && el !== document.documentElement && parts.length < 8) {
      if (el.id && word.test(el.id) && document.querySelectorAll('#' + el.id).length === 1) {
        parts.unshift('#' + el.id);
        break;
      }
      let part = el.localName;
      const classes = [...el.classList].filter((c) => word.test(c)).slice(0, 2);
      if (classes.length) part += '.' + classes.join('.');
      const parent = el.parentElement;
      if (parent) {
        const same = [...parent.children].filter((c) => c.localName === el.localName);
        if (same.length > 1) part += ':nth-of-type(' + (same.indexOf(el) + 1) + ')';
      }
      parts.unshift(part);
      const selector = parts.join(' > ');
      try {
        if (document.querySelectorAll(selector).length === 1) return selector;
      } catch (e) {}
      el = el.parentElement;
    }
    return parts.join(' > ');
  };

  const move = (e) => {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    if (!usable(el)) {
      target = null;
      outline.style.display = 'none';
      return;
    }
    target = el;
    const r = el.getBoundingClientRect();
    outline.style.cssText += ';display:block;left:' + r.left + 'px;top:' + r.top +
      'px;width:' + r.width + 'px;height:' + r.height + 'px';
  };
  const stop = () => {
    document.removeEventListener('mousemove', move, true);
    document.removeEventListener('click', click, true);
    document.removeEventListener('keydown', key, true);
    document.removeEventListener('contextmenu', stop, true);
    outline.remove();
    note.remove();
    delete window.__brujulaZap;
  };
  const click = (e) => {
    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();
    const el = target;
    stop();
    if (!usable(el)) return;
    const selector = path(el);
    el.style.setProperty('display', 'none', 'important');
    window.webkit.messageHandlers.brujulaZap.postMessage(selector);
  };
  const key = (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      stop();
    }
  };
  document.addEventListener('mousemove', move, true);
  document.addEventListener('click', click, true);
  document.addEventListener('keydown', key, true);
  document.addEventListener('contextmenu', stop, true);
  window.__brujulaZap = stop;
})();
