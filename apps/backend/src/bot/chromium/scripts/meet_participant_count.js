function() {
  const out = { count: null, source: null, raw: null };
  const isPresentationRow = (row) => {
    const childAria = [...row.querySelectorAll('[aria-label]')].map((el) =>
      (el.getAttribute('aria-label') || '').trim()
    );
    if (
      childAria.some(
        (label) => /\bpresentation\b/i.test(label) && /\b(mute|stop)\b/i.test(label)
      )
    ) {
      return true;
    }
    for (const el of row.querySelectorAll('span, div')) {
      if (el.closest('button, [role="button"]')) continue;
      const text = (el.textContent || '').trim();
      if (!text || text.length > 48) continue;
      if (el.querySelector('button, [role="button"]')) continue;
      if (/^(presentation|presenting|screen sharing)$/i.test(text)) return true;
    }
    return false;
  };
  try {
    const rosterRows = [...document.querySelectorAll('[role="listitem"][data-participant-id]')];
    const inCallRows = rosterRows.filter((row) => {
      const label = row.getAttribute('aria-label') || '';
      if (!label.trim()) {
        return false;
      }
      if (/\(\s*you\s*\)/i.test(label)) {
        return false;
      }
      if (isPresentationRow(row)) {
        return false;
      }
      const childLabels = [...row.querySelectorAll('[aria-label]')].map((el) =>
        (el.getAttribute('aria-label') || '').trim()
      );
      if (childLabels.some((text) => /^admit\b/i.test(text) || /^deny\b/i.test(text))) {
        return false;
      }
      return true;
    });
    if (inCallRows.length > 0) {
      out.count = inCallRows.length;
      out.source = 'listitem';
      return out;
    }

    const withLabels = document.querySelectorAll('[aria-label]');
    for (let i = 0; i < withLabels.length; i++) {
      const label = (withLabels[i].getAttribute('aria-label') || '').trim();
      if (!label) continue;
      let m = label.match(/(\d+)\s+(people|participants)/i);
      if (m) { out.count = parseInt(m[1], 10); out.source = 'aria_people'; out.raw = label; return out; }
      m = label.match(/(?:people|participants)\s*[-–—:]\s*(\d+)/i);
      if (m) { out.count = parseInt(m[1], 10); out.source = 'aria_dash'; out.raw = label; return out; }
      m = label.match(/(\d+)\s+joined/i);
      if (m) { out.count = parseInt(m[1], 10); out.source = 'aria_joined'; out.raw = label; return out; }
    }
    const body = document.body && document.body.innerText ? document.body.innerText : '';
    if (body) {
      const lower = body.toLowerCase();
      if (lower.includes("you're the only one here") || lower.includes('you\u2019re the only one here')) {
        out.count = 1;
        out.source = 'body_only_one';
        return out;
      }
      const m = body.match(/\b(\d+)\s+people\s+in\s+this\s+call\b/i);
      if (m) { out.count = parseInt(m[1], 10); out.source = 'body_text'; return out; }
    }
    const tiles = document.querySelectorAll('[data-participant-id], [data-requested-participant-id]');
    if (tiles.length) {
      const seen = {};
      for (let i = 0; i < tiles.length; i++) {
        const t = tiles[i];
        const id = t.getAttribute('data-participant-id') || t.getAttribute('data-requested-participant-id');
        if (id) seen[id] = true;
      }
      const n = Object.keys(seen).length;
      if (n > 0) { out.count = n; out.source = 'tiles'; return out; }
    }
  } catch (e) {
    out.source = 'error';
    out.raw = String(e && e.message ? e.message : e);
  }
  return out;
}
