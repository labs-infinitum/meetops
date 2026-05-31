async function() {
  function gmrMeetParticipantCount() {
    const out = { count: null, source: null, raw: null };
    try {
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

  function gmrVisibleText(node) {
    if (!node) return '';
    const text = (node.innerText || node.textContent || '').replace(/\s+/g, ' ').trim();
    return text.slice(0, 160);
  }

  function gmrParticipantNameFromTile(tile, fallbackId) {
    const labelled = tile.querySelector('[aria-label]');
    const labels = [
      tile.getAttribute('aria-label') || '',
      labelled ? (labelled.getAttribute('aria-label') || '') : '',
      gmrVisibleText(tile)
    ].filter(Boolean);
    for (const raw of labels) {
      const cleaned = raw
        .replace(/\b(muted|speaking|presenting|camera is off|microphone is off)\b/ig, '')
        .replace(/\b(pin|more options|participant|tile)\b/ig, '')
        .replace(/\s+/g, ' ')
        .trim();
      if (cleaned && !/^\d+$/.test(cleaned)) return cleaned.slice(0, 120);
    }
    return fallbackId || 'Unknown participant';
  }

  /** Participants from the Meet people panel (`[role="listitem"][data-participant-id]`). */
  function gmrListMeetParticipants() {
    const rows = [...document.querySelectorAll('[role="listitem"][data-participant-id]')];
    const isSelfRow = (row) => /\(\s*You\s*\)/i.test(row.textContent || '');
    const isNotInMeetingYet = (row) => {
      const ariaLabels = [...row.querySelectorAll('[aria-label]')]
        .map((el) => el.getAttribute('aria-label') || '');
      return ariaLabels.some((label) =>
        /^Admit\b/i.test(label) ||
        /^Deny\b/i.test(label) ||
        /^Ask to join\b/i.test(label)
      );
    };
    const getName = (row) => row.getAttribute('aria-label')?.trim() || '';
    const participants = rows
      .filter((row) => !isSelfRow(row))
      .filter((row) => !isNotInMeetingYet(row))
      .map((row) => ({
        id: row.getAttribute('data-participant-id'),
        name: getName(row),
        scrollTarget: row.getAttribute('data-scroll-target') || null
      }))
      .filter((p) => p.name && p.id);
    return [...new Map(participants.map((p) => [p.id, p])).values()];
  }

  function gmrExtractParticipants() {
    return gmrListMeetParticipants().map((p) => ({
      id: p.id,
      name: p.name,
      source: 'listitem',
      scrollTarget: p.scrollTarget,
      ariaLabel: p.name
    }));
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

  function gmrLooksLikeActiveSpeaker(tile) {
    const label = `${tile.getAttribute('aria-label') || ''} ${gmrVisibleText(tile)}`.toLowerCase();
    if (/\b(speaking|is speaking|active speaker)\b/.test(label)) return true;
    if (tile.querySelector('[aria-label*="speaking" i], [aria-label*="active speaker" i]')) return true;
    const className = typeof tile.className === 'string' ? tile.className.toLowerCase() : '';
    return /\b(active|speaking|speaker)\b/.test(className);
  }

  function gmrExtractActiveSpeaker(participants) {
    const tiles = Array.from(document.querySelectorAll('[data-participant-id], [data-requested-participant-id]'));
    for (const [index, tile] of tiles.entries()) {
      if (!gmrLooksLikeActiveSpeaker(tile)) continue;
      const rawId = tile.getAttribute('data-participant-id') || tile.getAttribute('data-requested-participant-id') || '';
      const name = gmrParticipantNameFromTile(tile, rawId || `tile:${index}`);
      const id = rawId || `name:${name.toLowerCase()}`;
      const participant = participants.find((p) => p.id === id);
      return {
        id,
        name: participant ? participant.name : name,
        source: rawId ? 'participant_id' : 'tile_name'
      };
    }
    return null;
  }

  function gmrCreateTimelineCollector(recordingStartedAtMs) {
    const state = {
      previousParticipants: new Map(),
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
      diffParticipants(participants) {
        const { events, current } = gmrDiffParticipantEvents(
          state.previousParticipants,
          participants,
          makeEvent
        );
        state.previousParticipants = current;
        return events;
      },
      collect() {
        const events = [];
        try {
          const participants = gmrExtractParticipants();
          events.push(...this.diffParticipants(participants));

          const activeSpeaker = gmrExtractActiveSpeaker(participants);
          const activeSpeakerId = activeSpeaker ? activeSpeaker.id : null;
          if (activeSpeakerId && activeSpeakerId !== state.activeSpeakerId) {
            events.push(makeEvent('active_speaker_changed', activeSpeaker, { source: activeSpeaker.source }));
          }
          state.activeSpeakerId = activeSpeakerId;

          state.snapshotCounter += 1;
          if (state.snapshotCounter === 1 || state.snapshotCounter % 6 === 0) {
            events.push(makeEvent('participant_snapshot', null, {
              count: participants.length,
              participants: participants.map((participant) => ({
                id: participant.id,
                name: participant.name,
                source: participant.source
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
        const participants = gmrExtractParticipants();
        const timelineEvents = window.__gmrTimelineCollector
          ? window.__gmrTimelineCollector.diffParticipants(participants)
          : [];
        const participantCount = participants.length;
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
