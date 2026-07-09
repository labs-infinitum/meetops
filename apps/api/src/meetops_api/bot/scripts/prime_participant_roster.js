async function() {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const PRIME_BUDGET_MS = 20000;
  const POLL_MS = 200;

  const result = {
    opened: false,
    closed: false,
    rosterCount: 0,
    phase: 'starting',
    error: null
  };

  const norm = (text) =>
    (text || '')
      .replace(/\u2019/g, "'")
      .replace(/\s+/g, ' ')
      .trim()
      .toLowerCase();

  const waitingForHostSignals = [
    'please wait until a meeting host brings you into the call',
    'please wait until a meeting host lets you in',
    'please wait until the host brings you into the call',
    'waiting for the host to let you in',
    'waiting for the meeting host',
    'the host will let you in soon'
  ];

  const isWaitingForHostAdmission = () => {
    const bodyText = norm(document.body ? document.body.innerText : '');
    return waitingForHostSignals.some((signal) => bodyText.includes(signal));
  };

  const hasPresentationStatusText = (text) => {
    const normalized = (text || '').replace(/\s+/g, ' ').trim();
    if (!normalized || normalized.length > 80) return false;
    if (/\b(leave call|ask to join|admit|deny)\b/i.test(normalized)) return false;
    return (
      /\bpresentation\b/i.test(normalized) ||
      /\bpresenting\b/i.test(normalized) ||
      /\bscreen sharing\b/i.test(normalized) ||
      /visitor\s*presentation/i.test(normalized)
    );
  };
  const isPresentationRow = (row) => {
    const childAria = [...row.querySelectorAll('[aria-label]')].map((el) =>
      (el.getAttribute('aria-label') || '').trim()
    );
    if (
      childAria.some(
        (label) =>
          /\bpresentation\b/i.test(label) && !/^visitor(\s+badge)?$/i.test(label)
      )
    ) {
      return true;
    }
    for (const el of row.querySelectorAll('span, div')) {
      if (el.closest('button, [role="button"]')) continue;
      if (el.querySelector('button, [role="button"]')) continue;
      if (hasPresentationStatusText(el.textContent)) return true;
    }
    return false;
  };

  const countRosterRows = () => {
    const rows = [...document.querySelectorAll('[role="listitem"][data-participant-id]')];
    return rows.filter((row) => {
      const label = (row.getAttribute('aria-label') || '').trim();
      return label && !isPresentationRow(row);
    }).length;
  };

  /** Accessible name from aria-labelledby, aria-label, and tooltip (no hardcoded Meet ids). */
  function controlAccessibleText(el) {
    if (!el) {
      return '';
    }
    const parts = [];
    const labelledBy = el.getAttribute('aria-labelledby');
    if (labelledBy) {
      for (const id of labelledBy.split(/\s+/)) {
        const node = document.getElementById(id);
        if (node) {
          parts.push(node.textContent || '');
        }
      }
    }
    parts.push(
      el.getAttribute('aria-label') || '',
      el.getAttribute('data-tooltip') || '',
      el.getAttribute('title') || ''
    );
    return parts
      .filter(Boolean)
      .join(' ')
      .replace(/\s+/g, ' ')
      .trim();
  }

  function isPeoplePanelControl(el) {
    const text = controlAccessibleText(el).toLowerCase();
    if (!text) {
      return false;
    }
    return (
      text === 'people' ||
      /^show everyone\b/.test(text) ||
      /^hide everyone\b/.test(text) ||
      /\bpeople\b.*\bjoined\b/.test(text) ||
      /^participants\b/.test(text)
    );
  }

  /** In-call People control: labelled "People" span, dialog popup, or accessible name. */
  function findParticipantPanelToggle() {
    for (const span of document.querySelectorAll('span')) {
      const label = (span.textContent || '').replace(/\s+/g, ' ').trim();
      if (label !== 'People') {
        continue;
      }
      const button =
        span.closest('[role="button"]') ||
        span.closest('button') ||
        (span.parentElement && span.parentElement.closest('[role="button"]'));
      if (button) {
        return button;
      }
    }

    for (const el of document.querySelectorAll('[role="button"][aria-haspopup="dialog"]')) {
      if (isPeoplePanelControl(el)) {
        return el;
      }
    }
    for (const el of document.querySelectorAll('button, [role="button"]')) {
      if (isPeoplePanelControl(el)) {
        return el;
      }
    }
    return null;
  }

  function isParticipantPanelExpanded() {
    const toggle = findParticipantPanelToggle();
    if (!toggle) {
      return false;
    }
    return toggle.getAttribute('aria-expanded') === 'true';
  }

  function toggleParticipantPanel() {
    const toggle = findParticipantPanelToggle();
    if (!toggle) {
      return false;
    }
    toggle.click();
    return true;
  }

  const startedAt = Date.now();
  const timeLeft = () => Math.max(0, PRIME_BUDGET_MS - (Date.now() - startedAt));

  try {
    while (timeLeft() > 0) {
      if (isWaitingForHostAdmission()) {
        result.phase = 'waiting_for_host';
        await sleep(Math.min(POLL_MS, timeLeft()));
        continue;
      }

      result.phase = 'in_call';
      const toggle = findParticipantPanelToggle();
      if (!toggle) {
        await sleep(Math.min(POLL_MS, timeLeft()));
        continue;
      }

      if (!isParticipantPanelExpanded()) {
        toggle.click();
        result.opened = true;
        await sleep(Math.min(350, timeLeft()));
      } else {
        result.opened = true;
      }

      const waitUntil = startedAt + PRIME_BUDGET_MS - 450;
      while (Date.now() < waitUntil) {
        result.rosterCount = countRosterRows();
        if (result.rosterCount > 0) {
          break;
        }
        await sleep(80);
      }

      if (isParticipantPanelExpanded()) {
        if (toggleParticipantPanel()) {
          result.closed = true;
        }
      } else {
        result.closed = true;
      }

      result.rosterCount = countRosterRows();
      if (result.opened && result.rosterCount > 0) {
        result.phase = 'primed';
        return result;
      }
      if (result.opened) {
        result.phase = 'roster_empty';
        return result;
      }
    }

    if (result.phase === 'waiting_for_host') {
      result.error = 'timed_out_waiting_for_host';
    } else if (!result.opened) {
      result.error = 'people_toggle_not_found';
    } else {
      result.error = 'roster_still_empty';
    }
  } catch (error) {
    result.error = String(error && error.message ? error.message : error);
  }

  return result;
}
