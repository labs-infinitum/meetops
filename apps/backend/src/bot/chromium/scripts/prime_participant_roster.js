async function() {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const PRIME_BUDGET_MS = 3000;

  const result = {
    opened: false,
    closed: false,
    rosterCount: 0,
    error: null
  };

  const countRosterRows = () => {
    const rows = [...document.querySelectorAll('[role="listitem"][data-participant-id]')];
    return rows.filter((row) => (row.getAttribute('aria-label') || '').trim()).length;
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

  /** In-call People control: role + dialog popup + accessible name (stable across Meet builds). */
  function findParticipantPanelToggle() {
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
    if (!isParticipantPanelExpanded()) {
      if (!toggleParticipantPanel()) {
        result.error = 'people_toggle_not_found';
        return result;
      }
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
  } catch (error) {
    result.error = String(error && error.message ? error.message : error);
  }

  return result;
}
