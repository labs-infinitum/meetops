async function() {
  /** Status copy on a presentation listitem (Chrome: "Presentation"; Safari: "Visitor" + "Presentation"). */
  function gmrHasPresentationStatusText(text) {
    const normalized = (text || '').replace(/\s+/g, ' ').trim();
    if (!normalized || normalized.length > 80) {
      return false;
    }
    if (/\b(leave call|ask to join|admit|deny)\b/i.test(normalized)) {
      return false;
    }
    return (
      /\bpresentation\b/i.test(normalized) ||
      /\bpresenting\b/i.test(normalized) ||
      /\bscreen sharing\b/i.test(normalized) ||
      /visitor\s*presentation/i.test(normalized)
    );
  }

  /** Screen-share roster row (separate listitem from the person; do not treat as join/leave). */
  function gmrIsPresentationRosterRow(row) {
    const childAria = [...row.querySelectorAll('[aria-label]')].map((el) =>
      (el.getAttribute('aria-label') || '').trim()
    );
    // Chrome: "Mute …'s presentation"; Safari: "You can't unmute someone else's presentation"
    if (
      childAria.some(
        (label) =>
          /\bpresentation\b/i.test(label) &&
          !/^visitor(\s+badge)?$/i.test(label)
      )
    ) {
      return true;
    }
    for (const el of row.querySelectorAll('span, div')) {
      if (el.closest('button, [role="button"]')) {
        continue;
      }
      if (el.querySelector('button, [role="button"]')) {
        continue;
      }
      if (gmrHasPresentationStatusText(el.textContent)) {
        return true;
      }
    }
    return false;
  }

  function gmrNormalizeName(name) {
    return (name || '').trim().toLowerCase();
  }

  /** Meet ids look like `spaces/{space}/devices/{device}` — same space, different device rows. */
  function gmrParticipantSpacePrefix(id) {
    if (!id) {
      return null;
    }
    const marker = '/devices/';
    const idx = id.indexOf(marker);
    return idx > 0 ? id.slice(0, idx) : null;
  }

  function gmrNormalizeAvatarUrl(row) {
    const img = row.querySelector('img[src]');
    if (!img || !img.src) {
      return null;
    }
    try {
      const url = new URL(img.src);
      return `${url.origin}${url.pathname}`;
    } catch {
      return img.src.split('?')[0];
    }
  }

  /**
   * Map a presentation listitem to the in-call person row (disambiguates duplicate names).
   * Order: unique name → same Meet space → avatar → nearest person row above in roster.
   */
  function gmrResolvePresenterParticipantId(presentation, people) {
    const targetName = gmrNormalizeName(presentation.presenterName || presentation.name);
    let candidates = people.filter((person) => gmrNormalizeName(person.name) === targetName);

    const presSpace = gmrParticipantSpacePrefix(presentation.id);
    if (presSpace && candidates.length > 1) {
      const inSpace = candidates.filter(
        (person) => gmrParticipantSpacePrefix(person.id) === presSpace
      );
      if (inSpace.length > 0) {
        candidates = inSpace;
      }
    }

    if (presentation.avatarUrl && candidates.length > 1) {
      const byAvatar = candidates.filter((person) => person.avatarUrl === presentation.avatarUrl);
      if (byAvatar.length > 0) {
        candidates = byAvatar;
      }
    }

    if (candidates.length > 1 && presentation.rowIndex != null) {
      const above = candidates.filter((person) => person.rowIndex < presentation.rowIndex);
      if (above.length > 0) {
        candidates = [
          above.reduce((nearest, person) =>
            person.rowIndex > nearest.rowIndex ? person : nearest
          )
        ];
      }
    }

    if (candidates.length === 1) {
      return {
        presenterParticipantId: candidates[0].id,
        presenterMatch: 'linked'
      };
    }
    if (candidates.length > 1) {
      return {
        presenterParticipantId: null,
        presenterMatch: 'ambiguous',
        presenterCandidateIds: candidates.map((person) => person.id)
      };
    }

    if (presentation.avatarUrl && presSpace) {
      const byAvatarSpace = people.filter(
        (person) =>
          person.avatarUrl === presentation.avatarUrl &&
          gmrParticipantSpacePrefix(person.id) === presSpace
      );
      if (byAvatarSpace.length === 1) {
        return {
          presenterParticipantId: byAvatarSpace[0].id,
          presenterMatch: 'avatar_space'
        };
      }
    }

    return { presenterParticipantId: null, presenterMatch: 'unresolved' };
  }

  function gmrScreenShareEventSubject(presentation) {
    return {
      id: presentation.presenterParticipantId || presentation.id,
      name: presentation.presenterName || presentation.name
    };
  }

  function gmrScreenSharePayload(presentation) {
    const payload = {
      source: presentation.source,
      presentationId: presentation.id,
      presenterParticipantId: presentation.presenterParticipantId || null,
      presenterName: presentation.presenterName || presentation.name,
      presenterMatch: presentation.presenterMatch || null,
      scrollTarget: presentation.scrollTarget || null
    };
    if (presentation.presenterCandidateIds && presentation.presenterCandidateIds.length > 0) {
      payload.presenterCandidateIds = presentation.presenterCandidateIds;
    }
    return payload;
  }

  /** In-call roster rows (`[role="listitem"][data-participant-id]`); primed once after join via `prime_participant_roster.js`. */
  function gmrParseMeetRoster() {
    const rows = [...document.querySelectorAll('[role="listitem"][data-participant-id]')];
    const isSelfRow = (row) => /\(\s*You\s*\)/i.test(row.textContent || '');
    const isNotInMeetingYet = (row) => {
      const ariaLabels = [...row.querySelectorAll('[aria-label]')].map(
        (el) => el.getAttribute('aria-label') || ''
      );
      return ariaLabels.some((label) =>
        /^Admit\b/i.test(label) ||
        /^Deny\b/i.test(label) ||
        /^Ask to join\b/i.test(label)
      );
    };
    const getName = (row) => row.getAttribute('aria-label')?.trim() || '';
    const people = [];
    const presentations = [];
    for (let rowIndex = 0; rowIndex < rows.length; rowIndex += 1) {
      const row = rows[rowIndex];
      if (isSelfRow(row) || isNotInMeetingYet(row)) {
        continue;
      }
      const id = row.getAttribute('data-participant-id');
      const name = getName(row);
      if (!id || !name) {
        continue;
      }
      const base = {
        id,
        name,
        source: 'listitem',
        scrollTarget: row.getAttribute('data-scroll-target') || null,
        ariaLabel: name,
        avatarUrl: gmrNormalizeAvatarUrl(row),
        rowIndex
      };
      if (gmrIsPresentationRosterRow(row)) {
        presentations.push({ ...base, presenterName: name });
      } else {
        people.push(base);
      }
    }
    const dedupe = (items) => [...new Map(items.map((p) => [p.id, p])).values()];
    const dedupedPeople = dedupe(people);
    const dedupedPresentations = dedupe(presentations).map((presentation) => ({
      ...presentation,
      ...gmrResolvePresenterParticipantId(presentation, dedupedPeople)
    }));
    return { people: dedupedPeople, presentations: dedupedPresentations };
  }

  function gmrListMeetParticipants() {
    return gmrParseMeetRoster().people;
  }

  function gmrExtractParticipants() {
    return gmrListMeetParticipants();
  }

  function gmrDiffParticipantEvents(previousParticipants, participants, makeEvent) {
    const events = [];
    const current = new Map(participants.map((participant) => [participant.id, participant]));
    for (const participant of participants) {
      if (!previousParticipants.has(participant.id)) {
        events.push(makeEvent('participant_joined', participant, {
          source: participant.source,
          scrollTarget: participant.scrollTarget || null,
          ariaLabel: participant.ariaLabel || participant.name
        }));
      }
    }
    for (const [id, participant] of previousParticipants.entries()) {
      if (!current.has(id)) {
        events.push(makeEvent('participant_left', participant, { source: participant.source }));
      }
    }
    return { events, current };
  }

  function gmrDiffScreenShareEvents(previousPresentations, presentations, makeEvent) {
    const events = [];
    const current = new Map(presentations.map((p) => [p.id, p]));
    for (const presentation of presentations) {
      if (!previousPresentations.has(presentation.id)) {
        events.push(
          makeEvent(
            'screen_share_started',
            gmrScreenShareEventSubject(presentation),
            gmrScreenSharePayload(presentation)
          )
        );
      }
    }
    for (const [id, presentation] of previousPresentations.entries()) {
      if (!current.has(id)) {
        events.push(
          makeEvent(
            'screen_share_stopped',
            gmrScreenShareEventSubject(presentation),
            gmrScreenSharePayload(presentation)
          )
        );
      }
    }
    return { events, current };
  }

  /** Active speaker from the same people list rows (IDs align with join/leave diffs). */
  function gmrExtractActiveSpeaker(participants) {
    const rows = [...document.querySelectorAll('[role="listitem"][data-participant-id]')];
    for (const row of rows) {
      const id = row.getAttribute('data-participant-id');
      if (!id) continue;
      const label = `${row.getAttribute('aria-label') || ''} ${row.textContent || ''}`;
      if (!/\b(speaking|is speaking|active speaker)\b/i.test(label)) continue;
      const participant = participants.find((p) => p.id === id);
      if (participant) {
        return { id: participant.id, name: participant.name, source: 'listitem' };
      }
    }
    return null;
  }

  function gmrCreateTimelineCollector(recordingStartedAtMs) {
    const state = {
      previousParticipants: new Map(),
      previousPresentations: new Map(),
      activeSpeakerId: null,
      snapshotCounter: 0
    };
    const makeEvent = (eventType, participant, payload) => {
      const nowMs = Date.now();
      return {
        eventType,
        occurredAt: Math.floor(nowMs / 1000),
        relativeMs: Math.max(0, nowMs - recordingStartedAtMs),
        participantId: participant && participant.id ? String(participant.id) : null,
        participantName: participant && participant.name ? String(participant.name) : null,
        payload: payload || {}
      };
    };
    return {
      diffRoster() {
        const { people, presentations } = gmrParseMeetRoster();
        const peopleDiff = gmrDiffParticipantEvents(
          state.previousParticipants,
          people,
          makeEvent
        );
        const shareDiff = gmrDiffScreenShareEvents(
          state.previousPresentations,
          presentations,
          makeEvent
        );
        state.previousParticipants = peopleDiff.current;
        state.previousPresentations = shareDiff.current;
        return [...peopleDiff.events, ...shareDiff.events];
      },
      collect() {
        const events = [];
        try {
          const { people, presentations } = gmrParseMeetRoster();
          events.push(...this.diffRoster());

          const activeSpeaker = gmrExtractActiveSpeaker(people);
          const activeSpeakerId = activeSpeaker ? activeSpeaker.id : null;
          if (activeSpeakerId && activeSpeakerId !== state.activeSpeakerId) {
            events.push(makeEvent('active_speaker_changed', activeSpeaker, { source: activeSpeaker.source }));
          }
          state.activeSpeakerId = activeSpeakerId;

          state.snapshotCounter += 1;
          if (state.snapshotCounter === 1 || state.snapshotCounter % 6 === 0) {
            events.push(makeEvent('participant_snapshot', null, {
              count: people.length,
              participants: people.map((participant) => ({
                id: participant.id,
                name: participant.name,
                source: participant.source
              })),
              presentationCount: presentations.length,
              presentations: presentations.map((presentation) => ({
                id: presentation.id,
                name: presentation.name,
                presenterName: presentation.presenterName || presentation.name,
                presenterParticipantId: presentation.presenterParticipantId || null,
                source: presentation.source
              })),
              activeSpeakerId
            }));
          }
        } catch (error) {
          events.push(makeEvent('collector_warning', null, {
            message: String(error && error.message ? error.message : error)
          }));
        }
        return events;
      }
    };
  }

  function gmrSendTimelineEvents(events) {
    if (!events || events.length === 0 || typeof window.__gmrRecordChunk !== 'function') return;
    const send = window.__gmrTimelinePending.then(async () => {
      window.__gmrRecordChunk(JSON.stringify({ kind: 'timeline_events', events }));
    }).catch((error) => {
      window.__gmrUploadErrors.push(String(error && error.message ? error.message : error));
    });
    window.__gmrTimelinePending = send;
    window.__gmrUploads.push(send);
  }
  window.__gmrSendTimelineEvents = gmrSendTimelineEvents;

  window.__gmrRecordingStatus = { status: 'waiting_for_click' };
  if (window.__gmrRecorder && window.__gmrRecorder.state === 'recording') {
    window.__gmrRecordingStatus = {
      status: 'recording',
      mimeType: window.__gmrRecorder.mimeType || '',
      audioTrackCount: window.__gmrRecorderStream ? window.__gmrRecorderStream.getAudioTracks().length : null,
      videoTrackCount: window.__gmrRecorderStream ? window.__gmrRecorderStream.getVideoTracks().length : null,
      audioSource: window.__gmrAudioSource || null,
      streamInfo: window.__gmrStreamInfo || null
    };
    return window.__gmrRecordingStatus;
  }

  let button = document.querySelector('#gmr-start-recording');
  if (!button) {
    button = document.createElement('button');
    button.id = 'gmr-start-recording';
    button.textContent = '';
    button.setAttribute('aria-label', 'Start AI recording');
    button.style.cssText = [
      'position:fixed',
      'z-index:2147483647',
      'left:50%',
      'top:50%',
      'width:240px',
      'height:120px',
      'padding:0',
      'border:0',
      'margin:0',
      'opacity:0',
      'background:transparent',
      'transform:translate(-50%, -50%)',
      'overflow:hidden',
      'pointer-events:auto'
    ].join(';');
    document.body.appendChild(button);
  }

  button.onclick = async () => {
    button.disabled = true;
    button.style.pointerEvents = 'none';
    window.__gmrRecordingStatus = { status: 'starting' };

    let stream;
    const displayMediaBase = {
      video: {
        displaySurface: 'browser',
        logicalSurface: true,
        cursor: 'always'
      },
      preferCurrentTab: true,
      selfBrowserSurface: 'include',
      surfaceSwitching: 'exclude'
    };
    try {
      stream = await navigator.mediaDevices.getDisplayMedia({
        ...displayMediaBase,
        audio: false
      });
    } catch (error) {
      window.__gmrRecordingStatus = {
        status: 'failed',
        name: error && error.name ? error.name : 'Error',
        message: error && error.message ? error.message : String(error)
      };
      button.disabled = false;
      button.style.pointerEvents = 'auto';
      return;
    }

    button.remove();

    const videoTracks = stream.getVideoTracks();
    const audioWarnings = [];
    const meetElementAudioTracks = [];
    try {
      for (const [elementIndex, element] of Array.from(document.querySelectorAll('audio,video')).entries()) {
        const streamLike = element.srcObject && typeof element.srcObject.getAudioTracks === 'function'
          ? element.srcObject
          : (typeof element.captureStream === 'function' ? element.captureStream() : null);
        if (!streamLike || typeof streamLike.getAudioTracks !== 'function') {
          continue;
        }
        for (const [trackIndex, track] of streamLike.getAudioTracks().entries()) {
          if (track.readyState === 'live') {
            meetElementAudioTracks.push({ track, label: `meetElement:${elementIndex}:${trackIndex}` });
          }
        }
      }
    } catch (elementAudioError) {
      audioWarnings.push(`Meet element audio unavailable: ${elementAudioError && elementAudioError.message ? elementAudioError.message : String(elementAudioError)}`);
    }

    const audioInputs = meetElementAudioTracks;
    const audioInputTracks = audioInputs.map((input) => input.track);
    let audioTracks = audioInputTracks;
    const audioSource = audioInputs.length > 0 ? 'meet_element_audio' : 'none';
    let audioContext = null;
    let audioMixDestination = null;
    let audioMixSources = [];
    if (audioInputTracks.length > 0) {
      const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
      if (AudioContextCtor) {
        audioContext = new AudioContextCtor();
        if (audioContext.state === 'suspended') {
          await audioContext.resume();
        }
        audioMixDestination = audioContext.createMediaStreamDestination();
        audioMixSources = audioInputs.map((input) => {
          const source = audioContext.createMediaStreamSource(new MediaStream([input.track]));
          source.connect(audioMixDestination);
          return source;
        });
        audioTracks = audioMixDestination.stream.getAudioTracks();
      }
    }
    const recordingStream = new MediaStream([...videoTracks, ...audioTracks]);
    const mimeTypes = audioTracks.length > 0
      ? [
          'video/webm;codecs=vp8,opus',
          'video/webm;codecs=vp9,opus',
          'video/webm'
        ]
      : [
          'video/webm;codecs=vp8',
          'video/webm;codecs=vp9',
          'video/webm'
        ];
    const mimeType = mimeTypes.find((candidate) => MediaRecorder.isTypeSupported(candidate)) || '';
    const streamInfo = {
      audioSource,
      meetElementAudioTracks: meetElementAudioTracks.map(({ track, label }) => ({ label, trackLabel: track.label, settings: track.getSettings ? track.getSettings() : {} })),
      audioTracks: audioTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} })),
      videoTracks: videoTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} }))
    };
    const recorder = new MediaRecorder(recordingStream, mimeType ? { mimeType } : undefined);
    const audioMimeTypes = [
      'audio/webm;codecs=opus',
      'audio/webm'
    ];
    const audioMimeType = audioTracks.length > 0
      ? (audioMimeTypes.find((candidate) => MediaRecorder.isTypeSupported(candidate)) || '')
      : '';
    const audioRecorder = audioTracks.length > 0
      ? new MediaRecorder(new MediaStream(audioTracks), audioMimeType ? { mimeType: audioMimeType } : undefined)
      : null;
    window.__gmrRecorder = recorder;
    window.__gmrAudioRecorder = audioRecorder;
    window.__gmrRecorderStream = recordingStream;
    window.__gmrCaptureStreams = [stream];
    window.__gmrAudioContext = audioContext;
    window.__gmrAudioMixDestination = audioMixDestination;
    window.__gmrAudioMixSources = audioMixSources;
    window.__gmrStreamInfo = streamInfo;
    window.__gmrAudioSource = audioSource;
    window.__gmrUploads = [];
    window.__gmrUploadErrors = [];
    for (const warning of audioWarnings) {
      window.__gmrUploadErrors.push(warning);
    }
    window.__gmrChunkCount = 0;
    window.__gmrAcceptedChunks = 0;
    window.__gmrUploadedBytes = 0;
    window.__gmrPendingUpload = Promise.resolve();
    window.__gmrPendingAudioUpload = Promise.resolve();
    window.__gmrTimelinePending = Promise.resolve();
    window.__gmrChunkBindingAvailable = typeof window.__gmrRecordChunk === 'function';
    if (audioTracks.length === 0) {
      window.__gmrUploadErrors.push('Meet media elements exposed no live audio tracks; recording video only');
    }

    window.__gmrRecordingStartedAtMs = Date.now();
    window.__gmrTimelineCollector = gmrCreateTimelineCollector(window.__gmrRecordingStartedAtMs);
    gmrSendTimelineEvents(window.__gmrTimelineCollector.collect());
    window.__gmrTimelineInterval = window.setInterval(() => {
      gmrSendTimelineEvents(window.__gmrTimelineCollector.collect());
    }, 5000);

    const blobToBase64 = async (blob) => {
      const bytes = new Uint8Array(await blob.arrayBuffer());
      let binary = '';
      const stride = 0x8000;
      for (let offset = 0; offset < bytes.length; offset += stride) {
        binary += String.fromCharCode.apply(null, bytes.subarray(offset, offset + stride));
      }
      return btoa(binary);
    };

    recorder.ondataavailable = (event) => {
      if (!event.data || event.data.size === 0) return;
      window.__gmrChunkCount += 1;
      const chunk = event.data;
      const upload = window.__gmrPendingUpload.then(async () => {
        if (typeof window.__gmrRecordChunk !== 'function') {
          throw new Error('recording DevTools binding is unavailable');
        }
        const data = await blobToBase64(chunk);
        const timelineEvents = window.__gmrTimelineCollector
          ? window.__gmrTimelineCollector.diffRoster()
          : [];
        const participantCount = gmrListMeetParticipants().length;
        window.__gmrLastParticipantCount = participantCount;
        window.__gmrLastParticipantSource = 'listitem';
        const chunkPayload = {
          kind: 'chunk',
          sequence: window.__gmrChunkCount,
          size: chunk.size,
          data,
          participantCount,
          participantSource: 'listitem'
        };
        if (timelineEvents.length > 0) {
          chunkPayload.timelineEvents = timelineEvents;
        }
        window.__gmrRecordChunk(JSON.stringify(chunkPayload));
        window.__gmrAcceptedChunks += 1;
        window.__gmrUploadedBytes += chunk.size;
      }).catch((error) => {
        window.__gmrUploadErrors.push(String(error && error.message ? error.message : error));
      });
      window.__gmrPendingUpload = upload;
      window.__gmrUploads.push(upload);
    };

    if (audioRecorder) {
      audioRecorder.ondataavailable = (event) => {
        if (!event.data || event.data.size === 0) return;
        const chunk = event.data;
        const upload = window.__gmrPendingAudioUpload.then(async () => {
          if (typeof window.__gmrRecordChunk !== 'function') {
            throw new Error('recording DevTools binding is unavailable');
          }
          const data = await blobToBase64(chunk);
          window.__gmrRecordChunk(JSON.stringify({
            kind: 'audio_chunk',
            size: chunk.size,
            data
          }));
        }).catch((error) => {
          window.__gmrUploadErrors.push(String(error && error.message ? error.message : error));
        });
        window.__gmrPendingAudioUpload = upload;
        window.__gmrUploads.push(upload);
      };
    }

    recorder.start(1000);
    if (audioRecorder) {
      audioRecorder.start(1000);
    }
    window.__gmrRecordingStatus = {
      status: 'recording',
      mimeType,
      streamInfo,
      audioTrackCount: audioTracks.length,
      videoTrackCount: videoTracks.length,
      audioSource
    };
  };

  return window.__gmrRecordingStatus;
}
