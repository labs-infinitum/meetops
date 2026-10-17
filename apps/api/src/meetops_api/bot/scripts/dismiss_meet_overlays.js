async function() {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const visible = (el) => {
    if (!el) return false;
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    return style.visibility !== 'hidden' && style.display !== 'none' && rect.width > 0 && rect.height > 0;
  };
  const norm = (text) =>
    (text || '')
      .replace(/\u2019/g, "'")
      .replace(/\s+/g, ' ')
      .trim()
      .toLowerCase();

  const DISMISS_LABELS = new Set([
    'got it',
    'ok',
    'okay',
    'dismiss',
    'close',
    'no thanks',
    'not now',
    'continue',
    'i understand',
    'understood',
    'done'
  ]);

  const OVERLAY_BODY_HINTS = [
    'others may see your video differently',
    'your self view tile may show less',
    'click the icon button on the bottom right of your tile'
  ];

  const result = { dismissed: false, clicked: [] };

  const controlLabel = (el) =>
    norm(
      [
        el.innerText,
        el.textContent,
        el.getAttribute('aria-label'),
        el.getAttribute('data-tooltip'),
        el.getAttribute('title')
      ]
        .filter(Boolean)
        .join(' ')
    );

  const isDismissControl = (el) => {
    const label = controlLabel(el);
    if (!label) return false;
    if (DISMISS_LABELS.has(label)) return true;
    if (label.startsWith('got it')) return true;
    return false;
  };

  const findDialogs = () =>
    [
      ...document.querySelectorAll(
        '[role="dialog"], [role="alertdialog"], [aria-modal="true"]'
      )
    ].filter(visible);

  const tryDismissIn = (root, scope, reason) => {
    const controls = [...root.querySelectorAll('button, [role="button"]')].filter(visible);
    for (const control of controls) {
      if (!isDismissControl(control)) continue;
      control.click();
      result.clicked.push({
        buttonText: controlLabel(control),
        scope,
        reason: reason || null
      });
      return true;
    }
    return false;
  };

  for (const dialog of findDialogs()) {
    if (tryDismissIn(dialog, 'dialog', 'modal')) {
      result.dismissed = true;
      await sleep(250);
    }
  }

  // Non-modal notices anchored to the toolbar (the bot has no camera/mic), e.g. "Camera not
  // found / Make sure that your camera is plugged in". They cover the recorded video. Close only
  // the notice itself: walk up from its text to the nearest container holding a close button.
  const NOTICE_HINTS = [
    'camera not found',
    'microphone not found',
    'make sure that your camera is plugged in',
    'make sure that your microphone is plugged in',
    'your camera is off',
    'your microphone is off',
    'camera is blocked',
    'microphone is blocked'
  ];
  const closeWord = (label) =>
    label === 'close' || label.startsWith('close ') || label === 'dismiss' || DISMISS_LABELS.has(label);
  const isCloseControl = (el) =>
    // accessible name on its own (icon buttons often render a glyph as their text) or combined
    ['aria-label', 'data-tooltip', 'title'].some((attr) => closeWord(norm(el.getAttribute(attr)))) ||
    closeWord(controlLabel(el));
  if (document.body) {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    const done = new Set();
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const text = norm(node.nodeValue);
      if (!text || !NOTICE_HINTS.some((hint) => text.includes(hint))) continue;
      let el = node.parentElement;
      for (let depth = 0; el && depth < 6; depth += 1, el = el.parentElement) {
        if (done.has(el) || !visible(el)) continue;
        const close = [...el.querySelectorAll('button, [role="button"]')].find(
          (c) => visible(c) && isCloseControl(c)
        );
        if (close) {
          close.click();
          done.add(el);
          result.dismissed = true;
          result.clicked.push({ buttonText: controlLabel(close), scope: 'notice', reason: text.slice(0, 60) });
          await sleep(250);
          break;
        }
      }
    }
  }

  const bodyText = norm(document.body ? document.body.innerText : '');
  const hintedOverlay = OVERLAY_BODY_HINTS.some((hint) => bodyText.includes(hint));
  if (hintedOverlay && tryDismissIn(document, 'page', 'body_hint')) {
    result.dismissed = true;
    await sleep(250);
  }

  return result;
}
