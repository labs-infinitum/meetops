async function() {
  const displayName = __DISPLAY_NAME_JSON__;
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
  const bodyText = norm(document.body ? document.body.innerText : '');
  const controls = Array.from(document.querySelectorAll('button, [role="button"], a')).filter(visible);
  const inputs = Array.from(document.querySelectorAll('input')).filter(visible);
  const controlText = (control) => norm([
    control.innerText,
    control.textContent,
    control.getAttribute('aria-label'),
    control.getAttribute('data-tooltip'),
    control.getAttribute('title')
  ].filter(Boolean).join(' '));
  const visibleControls = controls.map(controlText).filter(Boolean).slice(0, 20);
  const clickButton = (patterns) => {
    const control = controls.find((candidate) => patterns.some((pattern) => controlText(candidate).includes(pattern)));
    if (!control) return false;
    control.click();
    return true;
  };

  const waitingForHostSignals = [
    'please wait until a meeting host brings you into the call',
    'please wait until a meeting host lets you in',
    'please wait until the host brings you into the call',
    'waiting for the host to let you in',
    'waiting for the meeting host',
    'the host will let you in soon'
  ];

  if (bodyText.includes('you cannot join this video call') ||
      bodyText.includes('you were denied') ||
      bodyText.includes('no longer in the meeting') ||
      bodyText.includes('meeting is full')) {
    return { state: 'denied', message: document.body.innerText || 'Meet reports that the bot cannot join', title: document.title || '', controls: visibleControls };
  }

  if (clickButton(['continue without microphone and camera', 'continue without mic and camera'])) {
    await sleep(500);
    return { state: 'needs_action', message: 'Clicked pre-join no-mic/no-camera prompt', title: document.title || '', controls: visibleControls };
  }

  const nameInput = inputs.find((input) => {
    const label = norm([
      input.getAttribute('aria-label'),
      input.getAttribute('placeholder'),
      input.getAttribute('name')
    ].filter(Boolean).join(' '));
    return input.type !== 'hidden' && (label.includes('name') || label.includes('your name'));
  });
  if (nameInput && nameInput.value !== displayName) {
    nameInput.focus();
    nameInput.value = displayName;
    nameInput.dispatchEvent(new Event('input', { bubbles: true }));
    nameInput.dispatchEvent(new Event('change', { bubbles: true }));
    await sleep(250);
  }

  clickButton(['turn off microphone', 'mute microphone']);
  clickButton(['turn off camera', 'turn off video']);

  if (waitingForHostSignals.some((signal) => bodyText.includes(signal))) {
    return {
      state: 'waiting_for_host',
      message: 'Waiting for meeting host to admit the bot into the call',
      title: document.title || '',
      controls: visibleControls
    };
  }

  const joinedSignals = [
    'leave call',
    'show everyone',
    'meeting details',
    'people are in this call',
    'you’re the only one here',
    "you're the only one here"
  ];
  if (joinedSignals.some((signal) => bodyText.includes(signal)) ||
      controls.some((control) => controlText(control).includes('leave call'))) {
    return { state: 'joined', message: 'In-call UI detected (host admitted)', title: document.title || '', controls: visibleControls };
  }

  if (bodyText.includes('asking to be let in') ||
      bodyText.includes('waiting to be let in') ||
      bodyText.includes('you’ll join when someone lets you in') ||
      bodyText.includes("you'll join when someone lets you in")) {
    return { state: 'asked_to_join', message: 'Waiting for host admission', title: document.title || '', controls: visibleControls };
  }

  if (clickButton(['ask to join', 'join now'])) {
    await sleep(500);
    return { state: 'asked_to_join', message: 'Clicked join button', title: document.title || '', controls: visibleControls };
  }

  return { state: 'needs_action', message: 'Waiting for Meet join controls', title: document.title || '', controls: visibleControls };
}
