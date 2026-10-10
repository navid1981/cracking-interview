// Page extraction script, evaluated inside a Chrome tab via CDP `Runtime.evaluate`
// (with awaitPromise). Resolves to a JSON string. See docs/EXTRACT_TEXT_ENHANCEMENT_PLAN.md.
// Rules: extract the question, never the user's own answers (code editors excepted).
(async () => {
  const MAX_WAIT_MS = 300;
  const QUIET_MS = 80;
  const TEXT_CAP = 12000;
  const CODE_CAP = 20000;
  const FRAME_TEXT_CAP = 20000;
  const MIN_FRAME_SIDE = 100;
  const MIN_VISUAL_AREA = 40000;
  const MAX_IMAGES = 4;
  const NODE_BUDGET = 40000;
  const MIRROR_MIN_CHARS = 20;

  const EDITOR_SEL = '.monaco-editor, .CodeMirror, .cm-editor, .ace_editor';
  const SKIP_TAGS = new Set(['script', 'style', 'noscript', 'template', 'head', 'meta', 'link',
    'audio', 'video', 'object', 'embed', 'source', 'track', 'datalist', 'option', 'optgroup']);
  const JUNK_ROLE = new Set(['navigation', 'contentinfo']);
  const JUNK_ATTR_RE = /(^|[\s_-])(cookie|consent|gdpr|onetrust|cookiebot)/i;
  const ACTION_RE = /^(submit( answer| code)?|next( question)?|previous|prev|back|continue|skip|run( code)?|reset|cancel|ok|close|log ?in|sign ?in|sign ?up|register|share( on .+)?|save|clear|enlarge image|show more|show less|load more|upload code as file)$/i;
  const FRAME_DENY_RE = /recaptcha|hcaptcha|chilipiper|platform\.twitter\.com|twitter\.com\/widgets|facebook\.com\/plugins|intercom|hotjar|doubleclick|googlesyndication|googletagmanager|google-analytics|disqus|drift\.com|zendesk|hubspot|sprig|livechat|tawk\.to|crisp\.chat/i;
  const PLACEHOLDER_OPTION_RE = /^(-+|choose.*|select.*|please (choose|select).*|pick.*)$/i;
  const INVISIBLE_RE = /[​‎‏‪-‮⁦-⁩﻿]/g;
  // Inside <pre>-like text, spaces/tabs are swapped for these markers so normalization keeps them,
  // and every line gets PRE_LINE so empty or repeated code lines are not dropped.
  const PRE_SPACE = '\u0001';
  const PRE_TAB = '\u0002';
  const PRE_LINE = '\u0005';
  const EDITOR_TOKEN = (i) => `\u0003EDITOR${i}\u0003`;
  // Paragraph-level blocks get a blank line around them; other blocks just a line break.
  const PARA = '\u0004';
  const PARAGRAPH_TAGS = new Set(['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'table', 'pre', 'ul', 'ol',
    'blockquote', 'details', 'section', 'article', 'form', 'fieldset', 'figure', 'hr']);

  const waitForQuietDom = async () => {
    const start = performance.now();
    let lastChange = start;
    const observer = new MutationObserver(() => { lastChange = performance.now(); });
    try {
      observer.observe(document.documentElement, { subtree: true, childList: true, characterData: true });
    } catch (_) {
      return;
    }
    // MessageChannel ticks are not throttled like timers when the Chrome window is occluded.
    const channel = new MessageChannel();
    const tick = () => new Promise((resolve) => {
      channel.port1.onmessage = () => resolve();
      channel.port2.postMessage(0);
    });
    while (true) {
      const now = performance.now();
      if (now - start >= MAX_WAIT_MS || now - lastChange >= QUIET_MS) break;
      await tick();
    }
    observer.disconnect();
  };

  const styleOf = (el) => {
    const view = el.ownerDocument.defaultView;
    return view ? view.getComputedStyle(el) : null;
  };

  const isBlockDisplay = (display) => !(display.startsWith('inline') || display === 'contents' || display.startsWith('table-cell'));

  const rectOf = (el) => {
    const r = el.getBoundingClientRect();
    return { width: Math.round(r.width), height: Math.round(r.height), top: Math.round(r.top), left: Math.round(r.left) };
  };

  const isTiny = (el) => {
    const r = el.getBoundingClientRect();
    return r.width <= 2 || r.height <= 2;
  };

  const linearMath = (el) => {
    const leaves = Array.from(el.querySelectorAll('*')).filter((n) => !n.firstElementChild && n.localName !== 'annotation');
    const parts = (leaves.length ? leaves.map((n) => n.textContent) : [el.textContent]);
    return parts.map((p) => (p || '').trim()).filter(Boolean).join(' ').replace(/\s+/g, ' ');
  };

  // ── Frames ────────────────────────────────────────────────────────────────
  const stats = { framesRead: 0, framesSkipped: 0, nodes: 0, budgetHit: false };

  const frameDocument = (frame) => {
    const src = frame.getAttribute('src') || '';
    if (FRAME_DENY_RE.test(src)) return null;
    const r = frame.getBoundingClientRect();
    if (r.width < MIN_FRAME_SIDE || r.height < MIN_FRAME_SIDE) return null;
    const style = styleOf(frame);
    if (style && (style.display === 'none' || style.visibility === 'hidden')) return null;
    try {
      const doc = frame.contentDocument;
      if (!doc || !doc.body) return null;
      if (FRAME_DENY_RE.test(doc.URL || '')) return null;
      return doc;
    } catch (_) {
      return null; // cross-origin
    }
  };

  // ── Choice groups (radio/checkbox) → A) B) C) labels ───────────────────────
  const CHOICE_SEL = 'input[type="radio"], input[type="checkbox"], [role="radio"], [role="checkbox"]';
  const choiceGroups = new Map();
  const choiceGroupOf = new WeakMap();

  const indexChoices = (root) => {
    for (const el of root.querySelectorAll(CHOICE_SEL)) {
      if (el.localName === 'input' && el.name) {
        const scope = el.form || el.ownerDocument;
        const byName = choiceGroupOf.get(scope) || new Map();
        choiceGroupOf.set(scope, byName);
        const key = `${el.type}:${el.name}`;
        if (!byName.has(key)) byName.set(key, []);
        const list = byName.get(key);
        if (!list.includes(el)) list.push(el);
        choiceGroups.set(el, list);
        continue;
      }
      const container = el.closest('[role="radiogroup"], [role="group"], [role="listbox"], fieldset')
        || (el.parentElement && el.parentElement.parentElement) || el.parentElement || root;
      const byContainer = choiceGroupOf.get(container) || new Map();
      choiceGroupOf.set(container, byContainer);
      const kind = el.getAttribute('role') || el.type;
      if (!byContainer.has(kind)) byContainer.set(kind, []);
      const list = byContainer.get(kind);
      if (!list.includes(el)) list.push(el);
      choiceGroups.set(el, list);
    }
  };

  const choiceMarker = (el) => {
    const list = choiceGroups.get(el);
    if (!list || list.length === 1) return ' [ ] ';
    const i = list.indexOf(el);
    return i < 26 ? ` ${String.fromCharCode(65 + i)}) ` : ` ${i + 1}) `;
  };

  // ── Visual content (images, canvases, SVG drawings) ────────────────────────
  const imageLabel = new Map();
  const drawnLabel = new Map();
  const images = [];
  const drawn = [];

  const collectVisuals = (roots) => {
    const imgCandidates = [];
    for (const root of roots) {
      for (const el of root.querySelectorAll('img, canvas, svg')) {
        if (el.localName === 'svg' && el.parentElement && el.parentElement.closest('svg')) continue;
        if (el.closest(EDITOR_SEL)) continue;
        const rect = rectOf(el);
        const area = rect.width * rect.height;
        if (area < MIN_VISUAL_AREA) continue;
        const style = styleOf(el);
        if (style && (style.visibility === 'hidden' || style.display === 'none')) continue;
        if (el.localName === 'img') {
          if (!el.complete || !el.naturalWidth) continue;
          imgCandidates.push({ el, rect, area });
        } else {
          drawn.push({ kind: el.localName, ...rect });
          drawnLabel.set(el, true);
        }
      }
    }
    imgCandidates.sort((a, b) => b.area - a.area);
    const chosen = imgCandidates.slice(0, MAX_IMAGES);
    chosen.sort((a, b) => (a.el.compareDocumentPosition(b.el) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1));
    chosen.forEach((c, i) => {
      imageLabel.set(c.el, i + 1);
      images.push({
        index: i + 1,
        src: c.el.currentSrc || c.el.src,
        alt: (c.el.getAttribute('alt') || '').trim(),
        naturalWidth: c.el.naturalWidth,
        naturalHeight: c.el.naturalHeight,
        frameUrl: c.el.ownerDocument.URL,
        ...c.rect,
      });
    });
  };

  // ── Code editors ───────────────────────────────────────────────────────────
  const editors = [];
  const editorIndex = new WeakMap();

  const registerEditor = (el) => {
    if (editorIndex.has(el)) return editorIndex.get(el);
    const i = editors.length;
    editors.push({ el, language: '', value: '', source: '' });
    editorIndex.set(el, i);
    return i;
  };

  const renderedLines = (el, lineSel) => {
    const lines = Array.from(el.querySelectorAll(lineSel));
    lines.sort((a, b) => (parseFloat(a.style.top) || 0) - (parseFloat(b.style.top) || 0));
    return lines.map((l) => (l.textContent || '').replace(/ /g, ' ')).join('\n');
  };

  const readEditors = () => {
    const usedModels = new Set();
    const pendingMonaco = [];
    for (const ed of editors) {
      const el = ed.el;
      const win = el.ownerDocument.defaultView || window;
      try {
        if (el.matches('.monaco-editor')) {
          const api = win.monaco && win.monaco.editor;
          const instances = api && typeof api.getEditors === 'function' ? api.getEditors() : [];
          for (const inst of instances) {
            const dom = inst.getDomNode && inst.getDomNode();
            if (!dom || !(dom === el || el.contains(dom) || dom.contains(el))) continue;
            const model = inst.getModel && inst.getModel();
            if (!model) continue;
            usedModels.add(model);
            ed.value = model.getValue();
            ed.language = model.getLanguageId ? model.getLanguageId() : '';
            ed.source = 'monaco-api';
            break;
          }
          if (!ed.source) pendingMonaco.push(ed);
        } else if (el.matches('.CodeMirror') && el.CodeMirror) {
          ed.value = el.CodeMirror.getValue();
          ed.source = 'codemirror5-api';
        } else if (el.matches('.cm-editor')) {
          const content = el.querySelector('.cm-content');
          const view = (content && content.cmView && content.cmView.view) || (el.cmView && el.cmView.view);
          if (view && view.state) {
            ed.value = view.state.doc.toString();
            ed.source = 'codemirror6-api';
          } else {
            ed.value = renderedLines(el, '.cm-line');
            ed.source = 'rendered-lines';
          }
        } else if (el.matches('.ace_editor')) {
          const inst = (el.env && el.env.editor) || (win.ace && win.ace.edit && win.ace.edit(el));
          if (inst) {
            ed.value = inst.getValue();
            ed.source = 'ace-api';
          } else {
            ed.value = renderedLines(el, '.ace_line');
            ed.source = 'rendered-lines';
          }
        }
      } catch (_) {
        // fall through to the DOM fallbacks below
      }
    }

    // Monaco without getEditors(): hand out the remaining non-empty models, code languages first.
    if (pendingMonaco.length) {
      const win = pendingMonaco[0].el.ownerDocument.defaultView || window;
      const api = win.monaco && win.monaco.editor;
      let models = [];
      try {
        models = api && typeof api.getModels === 'function' ? api.getModels() : [];
      } catch (_) {
        models = [];
      }
      const spare = models
        .filter((m) => !usedModels.has(m) && m.getValue().trim())
        .sort((a, b) => Number(a.getLanguageId() === 'plaintext') - Number(b.getLanguageId() === 'plaintext'));
      for (const ed of pendingMonaco) {
        const model = spare.shift();
        if (model) {
          ed.value = model.getValue();
          ed.language = model.getLanguageId();
          ed.source = 'monaco-models';
          continue;
        }
        const ta = ed.el.querySelector('textarea');
        if (ta && ta.value.trim()) {
          ed.value = ta.value;
          ed.source = 'monaco-textarea';
        } else {
          ed.value = renderedLines(ed.el, '.view-line');
          ed.source = 'rendered-lines';
        }
      }
    }
    for (const ed of editors) {
      if (!ed.source && ed.el.matches('.CodeMirror')) {
        ed.value = renderedLines(ed.el, '.CodeMirror-line');
        ed.source = 'rendered-lines';
      }
      if (ed.value.length > CODE_CAP) ed.value = ed.value.slice(0, CODE_CAP) + '\n[... code truncated]';
    }
  };

  // ── DOM walker ─────────────────────────────────────────────────────────────
  // Menus, sidebars and footers without semantic tags are blocks made almost entirely of links.
  const linkStats = new Map();
  const indexLinks = (doc) => {
    for (const a of doc.querySelectorAll('a')) {
      const len = (a.textContent || '').replace(/\s+/g, '').length;
      for (let p = a.parentElement; p; p = p.parentElement) {
        const s = linkStats.get(p) || { count: 0, len: 0 };
        s.count++;
        s.len += len;
        linkStats.set(p, s);
      }
    }
  };
  const isLinkList = (el) => {
    const s = linkStats.get(el);
    if (!s || s.count < 5) return false;
    const total = (el.textContent || '').replace(/\s+/g, '').length;
    if (total < 15 || s.len / total < 0.75) return false;
    return !el.querySelector(`input, select, textarea, [role="radio"], [role="checkbox"], ${EDITOR_SEL}`);
  };

  const isJunkContainer = (el) => {
    const tag = el.localName;
    if (tag === 'nav' || tag === 'footer') return true;
    const role = el.getAttribute('role');
    if (role && JUNK_ROLE.has(role)) return true;
    const attrs = `${el.id || ''} ${typeof el.className === 'string' ? el.className : ''}`;
    return JUNK_ATTR_RE.test(attrs) || isLinkList(el);
  };

  const ownText = (el) => (el.innerText || el.textContent || '').replace(/\s+/g, ' ').trim();

  const walk = (node, out, ctx) => {
    if (stats.nodes > NODE_BUDGET) {
      stats.budgetHit = true;
      return;
    }
    if (node.nodeType === Node.TEXT_NODE) {
      if (!ctx.visible) return;
      const raw = (node.nodeValue || '').replace(/[\u0001-\u0005]/g, '');
      if (ctx.pre) {
        out.push(raw.replace(/ /g, PRE_SPACE).replace(/\t/g, PRE_TAB).split('\n').map((l) => l + PRE_LINE).join('\n'));
      } else {
        out.push(raw.replace(/\s+/g, ' '));
      }
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const el = node;
    stats.nodes++;
    const tag = el.localName;
    if (SKIP_TAGS.has(tag)) return;

    // Choice inputs are often display:none behind a styled label; still label them.
    if (tag === 'input' && (el.type === 'radio' || el.type === 'checkbox')) {
      out.push(choiceMarker(el));
      return;
    }

    if (el.matches(EDITOR_SEL)) {
      out.push(`\n${EDITOR_TOKEN(registerEditor(el))}\n`);
      return;
    }
    if (isJunkContainer(el)) return;

    const style = styleOf(el);
    if (!style || style.display === 'none') return;
    const visible = style.visibility === 'visible';
    const childCtx = { visible, pre: ctx.pre || /^(pre|break-spaces)/.test(style.whiteSpace) };

    // Math: prefer the TeX/MathML source over rendered glyphs.
    if (el.classList.contains('katex')) {
      const tex = el.querySelector('annotation[encoding="application/x-tex"]');
      const math = el.querySelector('math');
      const t = tex ? `$${tex.textContent.trim()}$` : (math ? linearMath(math) : '');
      if (t) out.push(` ${t} `);
      return;
    }
    if (tag === 'mjx-container' || el.classList.contains('MathJax_SVG') || el.classList.contains('MathJax') || el.classList.contains('MathJax_CHTML')) {
      const sib = el.nextElementSibling;
      if (sib && sib.localName === 'script' && /math\/tex/.test(sib.type)) {
        out.push(` $${sib.textContent.trim()}$ `);
        return;
      }
      const math = el.querySelector('math');
      const label = el.getAttribute('aria-label');
      const t = math ? linearMath(math) : (label || '');
      if (t) out.push(` ${t} `);
      return;
    }
    if (tag === 'math') {
      const t = linearMath(el);
      if (t) out.push(` ${t} `);
      return;
    }

    if (tag === 'img' || tag === 'canvas' || tag === 'svg') {
      if (imageLabel.has(el)) {
        out.push(` [image ${imageLabel.get(el)}] `);
      } else if (drawnLabel.has(el)) {
        out.push(' [chart/drawing: see screenshot] ');
      } else if (tag === 'img') {
        const alt = (el.getAttribute('alt') || '').trim();
        if (alt && el.getBoundingClientRect().width >= 48) out.push(` [image: ${alt}] `);
      }
      if (tag === 'svg' && visible) {
        const labels = Array.from(el.querySelectorAll('text')).map((t) => (t.textContent || '').trim()).filter(Boolean);
        if (labels.length) out.push(`\n${labels.join(' | ')}\n`);
      }
      return;
    }

    if (tag === 'input') {
      const type = (el.type || 'text').toLowerCase();
      if (['hidden', 'submit', 'button', 'reset', 'image', 'file', 'range', 'color', 'email', 'password', 'tel'].includes(type)) return;
      const hint = `${el.getAttribute('placeholder') || ''} ${el.getAttribute('aria-label') || ''}`;
      if (type === 'search' || el.getAttribute('role') === 'searchbox' || /search/i.test(hint)) return;
      // Type-ahead pickers (e.g. a site's language selector) are UI, not exam blanks.
      if (el.getAttribute('role') === 'combobox' || el.hasAttribute('aria-autocomplete')) return;
      if (visible) out.push(' [blank] ');
      return;
    }

    if (tag === 'select') {
      const options = Array.from(el.options || [])
        .map((o) => (o.label || o.textContent || '').replace(/\s+/g, ' ').trim())
        .filter((t, i) => t && !(i === 0 && PLACEHOLDER_OPTION_RE.test(t)));
      if (visible && options.length) out.push(` [choose: ${options.join(' | ')}] `);
      return;
    }

    // Answer boxes are skipped. Tiny editable elements are screen-reader/clipboard mirrors
    // that canvas apps (e.g. Figma) keep with the real text, so those are read.
    if (tag === 'textarea' || el.getAttribute('contenteditable') === 'true' || el.getAttribute('contenteditable') === '') {
      if (!isTiny(el)) return;
      const t = tag === 'textarea' ? el.value : el.textContent;
      if (t && t.trim().length >= MIRROR_MIN_CHARS) {
        out.push(`\n[Hidden text copy kept by the page, unordered:]\n${t.trim()}\n`);
      }
      return;
    }

    const role = el.getAttribute('role');
    if (tag === 'button' || role === 'button') {
      if (ACTION_RE.test(ownText(el))) return;
    }
    if (role === 'radio' || role === 'checkbox') {
      out.push(choiceMarker(el));
      // Custom radios are often an empty styled box next to the label text.
      if (!ownText(el)) return;
    }

    if (tag === 'iframe') {
      const doc = frameDocument(el);
      if (!doc) {
        stats.framesSkipped++;
        return;
      }
      indexChoices(doc);
      indexLinks(doc);
      const frameOut = [];
      walk(doc.body, frameOut, { visible: true, pre: false });
      const frameText = frameOut.join('');
      if (frameText.length > FRAME_TEXT_CAP) {
        stats.framesSkipped++;
        return;
      }
      stats.framesRead++;
      out.push(`\n${frameText}\n`);
      return;
    }

    if (tag === 'br') {
      out.push('\n');
      return;
    }
    // Keep exponents/indices readable: 10<sup>4</sup> → 10^4, x<sub>i</sub> → x_i.
    if (tag === 'sup' || tag === 'sub') out.push(tag === 'sup' ? '^' : '_');

    const block = isBlockDisplay(style.display);
    const cell = style.display.startsWith('table-cell');
    const blockBreak = PARAGRAPH_TAGS.has(tag) ? `\n${PARA}\n` : '\n';
    if (block) out.push(blockBreak);
    if (cell) out.push('\t');
    if (tag === 'details' && !el.open) out.push('\n[collapsed section]\n');

    if (el.shadowRoot) {
      indexChoices(el.shadowRoot);
      for (const child of el.shadowRoot.childNodes) walk(child, out, childCtx);
    } else if (tag === 'slot') {
      const assigned = el.assignedNodes({ flatten: true });
      for (const child of (assigned.length ? assigned : el.childNodes)) walk(child, out, childCtx);
    } else {
      for (const child of el.childNodes) walk(child, out, childCtx);
    }

    if (cell) out.push('\t');
    if (block) out.push(blockBreak);
  };

  // ── Normalization ──────────────────────────────────────────────────────────
  const CHOICE_ONLY_RE = /^([A-Z]\)|\d+\)|\[ \])$/;

  const normalize = (raw) => {
    const lines = raw
      .replace(INVISIBLE_RE, '')
      .replace(/ /g, ' ')
      .split('\n');
    const result = [];
    let lastText = -1;
    let pendingBlank = false;
    for (const line of lines) {
      if (line.trim() === PARA) {
        pendingBlank = result.length > 0;
        continue;
      }
      const parts = line.split('\t').map((p) => p.replace(/ +/g, ' ').trim()).filter((p) => p && p !== PARA);
      const merged = [];
      for (const part of parts) {
        const prev = merged[merged.length - 1];
        if (prev !== undefined && CHOICE_ONLY_RE.test(prev)) merged[merged.length - 1] = `${prev} ${part}`;
        else merged.push(part);
      }
      const text = merged.join(' | ');
      if (!text) continue;
      if (lastText >= 0 && CHOICE_ONLY_RE.test(result[lastText])) {
        result.length = lastText + 1;
        result[lastText] = `${result[lastText]} ${text}`;
        pendingBlank = false;
        continue;
      }
      if (lastText >= 0 && text === result[lastText]) continue;
      if (pendingBlank) result.push('');
      pendingBlank = false;
      result.push(text);
      lastText = result.length - 1;
    }
    return result
      .join('\n')
      .replace(new RegExp(PRE_LINE, 'g'), '')
      .replace(new RegExp(PRE_SPACE, 'g'), ' ')
      .replace(new RegExp(PRE_TAB, 'g'), '\t')
      .trim();
  };

  const pickRoot = (doc) => {
    const body = doc.body;
    if (!body) return { el: doc.documentElement, name: 'document' };
    const bodyLen = (body.innerText || '').length;
    for (const sel of ['main', '[role="main"]']) {
      for (const el of doc.querySelectorAll(sel)) {
        const r = el.getBoundingClientRect();
        if (!r.width || !r.height) continue;
        const len = (el.innerText || '').length;
        if (len >= 200 && len >= bodyLen * 0.3) return { el, name: sel };
      }
    }
    return { el: body, name: 'body' };
  };

  // ── Main ───────────────────────────────────────────────────────────────────
  try {
    await waitForQuietDom();

    const root = pickRoot(document);
    indexChoices(document);
    indexLinks(document);

    const visualRoots = [root.el];
    for (const frame of root.el.querySelectorAll('iframe')) {
      const doc = frameDocument(frame);
      if (doc) visualRoots.push(doc.body);
    }
    collectVisuals(visualRoots);

    const out = [];
    walk(root.el, out, { visible: true, pre: false });

    // Editors outside the chosen root (e.g. a side-by-side layout) still count.
    for (const el of document.querySelectorAll(EDITOR_SEL)) {
      if (el.parentElement && el.parentElement.closest(EDITOR_SEL)) continue;
      if (!editorIndex.has(el)) out.push(`\n${EDITOR_TOKEN(registerEditor(el))}\n`);
    }

    readEditors();

    let text = normalize(out.join(''));
    const code = [];
    editors.forEach((ed, i) => {
      const keep = ed.value.trim().length > 0;
      if (keep) code.push({ number: code.length + 1, language: ed.language, source: ed.source, value: ed.value });
      const replacement = keep ? `[code editor ${code.length}: see below]` : '';
      text = text.split(EDITOR_TOKEN(i)).join(replacement);
    });
    text = text.replace(/\n{3,}/g, '\n\n').trim();

    let truncated = false;
    if (text.length > TEXT_CAP) {
      const cut = text.lastIndexOf('\n', TEXT_CAP);
      text = `${text.slice(0, cut > TEXT_CAP * 0.8 ? cut : TEXT_CAP)}\n[... page text truncated]`;
      truncated = true;
    }

    const codeBlocks = code.map((c) =>
      `--- Code editor ${c.number}${c.language ? ` (${c.language})` : ''} ---\n${c.value}\n--- End of code editor ${c.number} ---`);
    const fullText = [text, ...codeBlocks].filter(Boolean).join('\n\n');

    return JSON.stringify({
      version: 1,
      text: fullText,
      pageTextLength: text.length,
      code,
      images,
      drawn,
      truncated: truncated || stats.budgetHit,
      root: root.name,
      framesRead: stats.framesRead,
      framesSkipped: stats.framesSkipped,
      url: location.href,
      title: document.title,
    });
  } catch (e) {
    const fallback = (document.body ? document.body.innerText : '').slice(0, TEXT_CAP);
    return JSON.stringify({
      version: 1,
      text: fallback,
      pageTextLength: fallback.length,
      code: [],
      images: [],
      drawn: [],
      truncated: false,
      root: 'fallback',
      framesRead: 0,
      framesSkipped: 0,
      url: location.href,
      title: document.title,
      error: String(e && e.stack ? e.stack : e),
    });
  }
})()
