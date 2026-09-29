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

  function gmrFindParticipantById(participants, id) {
    const exact = participants.find((participant) => participant.id === id);
    if (exact) {
      return exact;
    }

    const space = gmrParticipantSpacePrefix(id);
    if (!space) {
      return null;
    }
    const sameSpace = participants.filter(
      (participant) => gmrParticipantSpacePrefix(participant.id) === space
    );
    return sameSpace.length === 1 ? sameSpace[0] : null;
  }

  function gmrTileName(tile) {
    const moreOptions = tile.querySelector('[aria-label^="More options for "]');
    if (moreOptions) {
      return moreOptions.getAttribute('aria-label').replace(/^More options for\s+/, '').trim();
    }
    const pin = tile.querySelector('[aria-label^="Pin "]');
    if (pin) {
      return pin
        .getAttribute('aria-label')
        .replace(/^Pin\s+/, '')
        .replace(/\s+to your main screen$/, '')
        .trim();
    }
    const mute = tile.querySelector('[aria-label^="Mute "]');
    if (mute) {
      return mute
        .getAttribute('aria-label')
        .replace(/^Mute\s+/, '')
        .replace(/'s microphone$/, '')
        .trim();
    }
    return '';
  }

  function gmrPseudoStyleVisible(style) {
    if (!style) {
      return false;
    }
    const opacity = Number(style.opacity || '0');
    return (
      opacity > 0.5 &&
      style.content !== 'none' &&
      style.display !== 'none' &&
      style.visibility !== 'hidden'
    );
  }

  function gmrElementCoversTile(elementRect, tileRect) {
    if (!elementRect.width || !elementRect.height || !tileRect.width || !tileRect.height) {
      return false;
    }
    return (
      elementRect.width >= tileRect.width * 0.45 &&
      elementRect.height >= tileRect.height * 0.45
    );
  }

  function gmrTileHasVisibleSpeakingEffect(tile) {
    const tileRect = tile.getBoundingClientRect();
    if (!tileRect.width || !tileRect.height) {
      return false;
    }
    for (const element of tile.querySelectorAll('*')) {
      if (element.closest('button, [role="button"]')) {
        continue;
      }
      if (!gmrElementCoversTile(element.getBoundingClientRect(), tileRect)) {
        continue;
      }
      if (
        gmrPseudoStyleVisible(getComputedStyle(element, '::before')) ||
        gmrPseudoStyleVisible(getComputedStyle(element, '::after'))
      ) {
        return true;
      }
    }
    return false;
  }

  function gmrTileHasSpeakingBorder(tile) {
    const style = getComputedStyle(tile);
    const outline = `${style.outlineColor || ''} ${style.boxShadow || ''} ${style.borderColor || ''}`;
    // Meet speaking ring is typically a saturated blue/green glow or outline.
    if (/rgb\(\s*(26|66|26)\s*,\s*(115|170|115)/i.test(outline)) {
      return true;
    }
    if (/rgba?\(\s*26\s*,\s*115\s*,\s*232/i.test(outline)) {
      return true;
    }
    if (Number(style.outlineWidth || '0') >= 2 && style.outlineStyle !== 'none') {
      return true;
    }
    return false;
  }

  function gmrTileHasSpeakingAria(tile) {
    const labels = [
      tile.getAttribute('aria-label') || '',
      ...[...tile.querySelectorAll('[aria-label]')].map((el) => el.getAttribute('aria-label') || '')
    ]
      .join(' ')
      .toLowerCase();
    if (!labels) {
      return false;
    }
    return (
      labels.includes('speaking') ||
      labels.includes('is talking') ||
      labels.includes('currently speaking')
    );
  }

  function gmrTileLooksLikeSpeaking(tile) {
    return (
      gmrTileHasVisibleSpeakingEffect(tile) ||
      gmrTileHasSpeakingBorder(tile) ||
      gmrTileHasSpeakingAria(tile)
    );
  }

  /** Active speaker from Meet tile visual state. Prefer named roster matches. */
  function gmrExtractActiveSpeaker(participants) {
    const tileIds = [
      ...document.querySelectorAll('[data-participant-id][data-tile-media-id]')
    ];
    for (const tileId of tileIds) {
      const id = tileId.getAttribute('data-participant-id');
      const tile = tileId.parentElement;
      if (!id || !tile || !gmrTileLooksLikeSpeaking(tile)) {
        continue;
      }
      const participant = gmrFindParticipantById(participants, id);
      const tileName = gmrTileName(tile);
      const name = (participant && participant.name) || tileName;
      if (!name) {
        continue;
      }
      return {
        id: (participant && participant.id) || id,
        name,
        source: participant ? 'tile_visual_effect' : 'tile_name_only'
      };
    }
    return null;
  }

  function gmrCreateTimelineCollector(recordingStartedAtMs) {
    const state = {
      previousParticipants: new Map(),
      previousPresentations: new Map(),
      activeSpeakerId: null,
      activeSpeakerName: null,
      lastSampleAtMs: 0,
      snapshotCounter: 0
    };
    const makeEvent = (eventType, participant, payload) => {
      const nowMs = Date.now();
      const name =
        participant && participant.name
          ? String(participant.name).trim()
          : null;
      return {
        eventType,
        occurredAt: Math.floor(nowMs / 1000),
        relativeMs: Math.max(0, nowMs - recordingStartedAtMs),
        participantId: participant && participant.id ? String(participant.id) : null,
        participantName: name || null,
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
      collectActiveSpeaker() {
        const events = [];
        try {
          const { people } = gmrParseMeetRoster();
          const activeSpeaker = gmrExtractActiveSpeaker(people);
          const activeSpeakerId = activeSpeaker ? activeSpeaker.id : null;
          const nowMs = Date.now();
          if (activeSpeakerId && activeSpeakerId !== state.activeSpeakerId) {
            events.push(
              makeEvent('active_speaker_changed', activeSpeaker, {
                source: activeSpeaker.source
              })
            );
            state.lastSampleAtMs = nowMs;
          } else if (
            activeSpeakerId &&
            activeSpeaker.name &&
            nowMs - state.lastSampleAtMs >= 1000
          ) {
            // Dense samples while the same person keeps speaking (helps overlap mapping).
            events.push(
              makeEvent('active_speaker_sample', activeSpeaker, {
                source: activeSpeaker.source
              })
            );
            state.lastSampleAtMs = nowMs;
          }
          state.activeSpeakerId = activeSpeakerId;
          state.activeSpeakerName = activeSpeaker ? activeSpeaker.name : null;
        } catch (error) {
          events.push(makeEvent('collector_warning', null, {
            message: String(error && error.message ? error.message : error)
          }));
        }
        return events;
      },
      collect() {
        const events = [];
        try {
          const { people, presentations } = gmrParseMeetRoster();
          events.push(...this.diffRoster());

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
              activeSpeakerId: state.activeSpeakerId,
              activeSpeakerName: state.activeSpeakerName
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

  // Audio levels (dBFS) used to pick the live source and to report capture health.
  const GMR_AUDIO_SIGNAL_DB = -55;
  const GMR_AUDIO_SILENT_DB = -70;
  // Seconds of "other bus has signal, active bus is silent" before switching.
  const GMR_AUDIO_SWITCH_EVIDENCE_SECS = 5;
  const GMR_AUDIO_HEALTH_REPORT_SECS = 15;

  function gmrToDb(value) {
    return value > 0 ? Math.max(-120, 20 * Math.log10(value)) : -120;
  }

  /**
   * Mixes candidate audio sources into `destination` with exactly one bus audible:
   * - `meet`: remote WebRTC tracks taken from Meet's <audio>/<video> srcObject. These carry
   *   participant audio at full level regardless of tab playback volume or output device.
   * - `tab`: getDisplayMedia tab audio (fallback; can arrive heavily attenuated).
   * Meet swaps tracks during a call, so the meet bus is re-synced every second.
   */
  function gmrCreateAudioMixer(audioContext, destination, excludedTrackIds) {
    const makeBus = (name) => {
      const input = audioContext.createGain();
      const analyser = audioContext.createAnalyser();
      analyser.fftSize = 2048;
      const output = audioContext.createGain();
      output.gain.value = 0;
      input.connect(analyser);
      input.connect(output);
      output.connect(destination);
      return {
        name,
        input,
        analyser,
        output,
        sources: new Map(),
        buffer: new Float32Array(analyser.fftSize),
        secondMaxDb: -120,
        window: { sumPower: 0, samples: 0, maxDb: -120, signalSecs: 0 }
      };
    };
    const buses = { meet: makeBus('meet'), tab: makeBus('tab') };
    let active = null;
    let evidence = 0;
    let switches = 0;
    let lastSwitchReason = null;
    let ticks = 0;
    let windowSecs = 0;
    let intervalId = null;

    const removeTrack = (bus, trackId) => {
      const entry = bus.sources.get(trackId);
      if (!entry) return;
      try {
        entry.source.disconnect();
      } catch (error) {
        // already disconnected
      }
      bus.sources.delete(trackId);
    };

    const addTrack = (bus, track, label) => {
      if (!track || track.kind !== 'audio' || track.readyState !== 'live') return false;
      if (excludedTrackIds.has(track.id) || bus.sources.has(track.id)) return false;
      const source = audioContext.createMediaStreamSource(new MediaStream([track]));
      source.connect(bus.input);
      bus.sources.set(track.id, { source, track, label });
      track.addEventListener('ended', () => removeTrack(bus, track.id), { once: true });
      return true;
    };

    const setActive = (key, reason) => {
      if (active === key) return;
      const now = audioContext.currentTime;
      for (const [busKey, bus] of Object.entries(buses)) {
        bus.output.gain.setTargetAtTime(busKey === key ? 1 : 0, now, 0.05);
      }
      if (active !== null) switches += 1;
      active = key;
      lastSwitchReason = reason;
      evidence = 0;
    };

    const syncMeetTracks = () => {
      let added = 0;
      for (const [elementIndex, element] of Array.from(document.querySelectorAll('audio,video')).entries()) {
        const stream = element.srcObject;
        if (!stream || typeof stream.getAudioTracks !== 'function') continue;
        for (const track of stream.getAudioTracks()) {
          if (addTrack(buses.meet, track, `meetElement:${elementIndex}`)) added += 1;
        }
      }
      for (const [trackId, entry] of buses.meet.sources) {
        if (entry.track.readyState === 'ended') removeTrack(buses.meet, trackId);
      }
      return added;
    };

    const liveCount = (bus) => {
      let count = 0;
      for (const entry of bus.sources.values()) {
        if (entry.track.readyState === 'live') count += 1;
      }
      return count;
    };

    const sampleBus = (bus) => {
      bus.analyser.getFloatTimeDomainData(bus.buffer);
      let sumSquares = 0;
      for (let i = 0; i < bus.buffer.length; i += 1) {
        sumSquares += bus.buffer[i] * bus.buffer[i];
      }
      const power = sumSquares / bus.buffer.length;
      const db = gmrToDb(Math.sqrt(power));
      bus.secondMaxDb = Math.max(bus.secondMaxDb, db);
      bus.window.sumPower += power;
      bus.window.samples += 1;
      bus.window.maxDb = Math.max(bus.window.maxDb, db);
    };

    const busReport = (bus) => ({
      tracks: liveCount(bus),
      avgDb: Math.round(gmrToDb(Math.sqrt(bus.window.samples ? bus.window.sumPower / bus.window.samples : 0))),
      maxDb: Math.round(bus.window.maxDb),
      signalSecs: bus.window.signalSecs
    });

    const report = (final) => {
      const payload = {
        kind: 'audio_health',
        final: !!final,
        active,
        switches,
        lastSwitchReason,
        windowSecs,
        meet: busReport(buses.meet),
        tab: busReport(buses.tab)
      };
      window.__gmrAudioHealth = payload;
      for (const bus of Object.values(buses)) {
        bus.window = { sumPower: 0, samples: 0, maxDb: -120, signalSecs: 0 };
      }
      windowSecs = 0;
      try {
        if (typeof window.__gmrRecordChunk === 'function') {
          window.__gmrRecordChunk(JSON.stringify(payload));
        }
      } catch (error) {
        window.__gmrUploadErrors.push(`audio health report failed: ${error && error.message ? error.message : error}`);
      }
    };

    const evaluateSecond = () => {
      for (const bus of Object.values(buses)) {
        if (bus.secondMaxDb >= GMR_AUDIO_SIGNAL_DB) bus.window.signalSecs += 1;
      }
      const meetLive = liveCount(buses.meet);
      const tabLive = liveCount(buses.tab);
      if (active === 'meet' && meetLive === 0 && tabLive > 0) {
        setActive('tab', 'meet_tracks_gone');
      } else if (active === 'tab' && tabLive === 0 && meetLive > 0) {
        setActive('meet', 'tab_audio_gone');
      } else {
        const other = active === 'meet' ? 'tab' : 'meet';
        const otherLive = other === 'meet' ? meetLive : tabLive;
        if (
          otherLive > 0 &&
          buses[other].secondMaxDb >= GMR_AUDIO_SIGNAL_DB &&
          buses[active].secondMaxDb <= GMR_AUDIO_SILENT_DB
        ) {
          evidence += 1;
        } else {
          evidence = Math.max(0, evidence - 1);
        }
        if (evidence >= GMR_AUDIO_SWITCH_EVIDENCE_SECS) {
          setActive(other, `${active}_silent_while_${other}_has_signal`);
        }
      }
      for (const bus of Object.values(buses)) {
        bus.secondMaxDb = -120;
      }
      windowSecs += 1;
    };

    return {
      setTabTracks(tracks) {
        tracks.forEach((track, index) => addTrack(buses.tab, track, `displayMedia:${index}`));
      },
      start() {
        syncMeetTracks();
        if (liveCount(buses.meet) > 0) {
          setActive('meet', 'initial_meet_tracks');
        } else if (liveCount(buses.tab) > 0) {
          setActive('tab', 'initial_no_meet_tracks');
        } else {
          setActive('meet', 'initial_no_sources');
        }
        // Sample 4x per second: the analyser only sees ~43ms per read.
        intervalId = window.setInterval(() => {
          try {
            ticks += 1;
            sampleBus(buses.meet);
            sampleBus(buses.tab);
            if (ticks % 4 === 0) {
              syncMeetTracks();
              evaluateSecond();
              if (windowSecs >= GMR_AUDIO_HEALTH_REPORT_SECS) report(false);
            }
          } catch (error) {
            window.__gmrUploadErrors.push(`audio mixer tick failed: ${error && error.message ? error.message : error}`);
          }
        }, 250);
      },
      stop() {
        if (intervalId !== null) {
          window.clearInterval(intervalId);
          intervalId = null;
        }
        report(true);
      },
      describe() {
        return {
          active,
          lastSwitchReason,
          meetTracks: [...buses.meet.sources.values()].map(({ track, label }) => ({ label, trackLabel: track.label, readyState: track.readyState })),
          tabTracks: [...buses.tab.sources.values()].map(({ track, label }) => ({ label, trackLabel: track.label, readyState: track.readyState }))
        };
      }
    };
  }

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
        cursor: 'always',
        width: { ideal: 1920, max: 1920 },
        height: { ideal: 1080, max: 1080 },
        frameRate: { ideal: 30, max: 30 }
      },
      preferCurrentTab: true,
      selfBrowserSurface: 'include',
      surfaceSwitching: 'exclude',
      // Chromium: include system/tab audio when the share sheet allows it
      systemAudio: 'include'
    };
    try {
      stream = await navigator.mediaDevices.getDisplayMedia({
        ...displayMediaBase,
        audio: {
          // Prefer tab audio from the capture sheet ("Also share tab audio")
          suppressLocalAudioPlayback: false
        }
      });
    } catch (audioError) {
      try {
        stream = await navigator.mediaDevices.getDisplayMedia({
          ...displayMediaBase,
          audio: true
        });
      } catch (error) {
        try {
          stream = await navigator.mediaDevices.getDisplayMedia({
            ...displayMediaBase,
            audio: false
          });
        } catch (finalError) {
          window.__gmrRecordingStatus = {
            status: 'failed',
            name: finalError && finalError.name ? finalError.name : 'Error',
            message: finalError && finalError.message ? finalError.message : String(finalError)
          };
          button.disabled = false;
          button.style.pointerEvents = 'auto';
          return;
        }
      }
    }

    button.remove();

    const videoTracks = stream.getVideoTracks();
    for (const track of videoTracks) {
      try {
        await track.applyConstraints({
          width: { ideal: 1920, max: 1920 },
          height: { ideal: 1080, max: 1080 },
          frameRate: { ideal: 30, max: 30 }
        });
      } catch (constraintError) {
        // Best-effort; some Chromium builds ignore constraints on display tracks
      }
    }
    const displayAudioTracks = stream.getAudioTracks().filter((track) => track.readyState === 'live');
    const audioWarnings = [];
    window.__gmrUploadErrors = [];

    // Everything goes through the mixer so tracks Meet adds mid-call are still recorded.
    // Without WebAudio, fall back to raw tab audio.
    let audioTracks = displayAudioTracks;
    let audioSource = displayAudioTracks.length > 0 ? 'display_media_tab_audio' : 'none';
    let audioContext = null;
    let audioMixDestination = null;
    let audioMixer = null;
    const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
    if (AudioContextCtor) {
      try {
        audioContext = new AudioContextCtor({ sampleRate: 48000 });
        if (audioContext.state === 'suspended') {
          await audioContext.resume();
        }
        audioMixDestination = audioContext.createMediaStreamDestination();
        const captureTrackIds = new Set(stream.getTracks().map((track) => track.id));
        audioMixer = gmrCreateAudioMixer(audioContext, audioMixDestination, captureTrackIds);
        audioMixer.setTabTracks(displayAudioTracks);
        audioMixer.start();
        audioTracks = audioMixDestination.stream.getAudioTracks();
        audioSource = 'mixer';
      } catch (mixerError) {
        audioWarnings.push(`Audio mixer unavailable, using raw tab audio: ${mixerError && mixerError.message ? mixerError.message : String(mixerError)}`);
        audioMixer = null;
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
      mixer: audioMixer ? audioMixer.describe() : null,
      displayAudioTracks: displayAudioTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} })),
      audioTracks: audioTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} })),
      videoTracks: videoTracks.map((track) => ({ label: track.label, settings: track.getSettings ? track.getSettings() : {} }))
    };
    const recorderOptions = {
      ...(mimeType ? { mimeType } : {}),
      videoBitsPerSecond: 4_000_000,
      audioBitsPerSecond: 128_000
    };
    const recorder = new MediaRecorder(recordingStream, recorderOptions);
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
    window.__gmrAudioMixer = audioMixer;
    window.__gmrStreamInfo = streamInfo;
    window.__gmrAudioSource = audioSource;
    window.__gmrUploads = [];
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
      window.__gmrUploadErrors.push('No audio source available (no tab audio, no WebAudio mixer); recording video only');
    }

    window.__gmrRecordingStartedAtMs = Date.now();
    window.__gmrTimelineCollector = gmrCreateTimelineCollector(window.__gmrRecordingStartedAtMs);
    gmrSendTimelineEvents(window.__gmrTimelineCollector.collect());
    gmrSendTimelineEvents(window.__gmrTimelineCollector.collectActiveSpeaker());
    window.__gmrActiveSpeakerInterval = window.setInterval(() => {
      gmrSendTimelineEvents(window.__gmrTimelineCollector.collectActiveSpeaker());
    }, 300);
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
