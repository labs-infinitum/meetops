async function() {
  const recorder = window.__gmrRecorder;
  if (!recorder) return { status: 'not_started', errors: [] };

  const stopRecorder = async (target) => {
    if (!target || target.state === 'inactive') return;
    await new Promise((resolve) => {
      const done = () => resolve();
      target.addEventListener('stop', done, { once: true });
      target.requestData();
      target.stop();
    });
  };

  await stopRecorder(window.__gmrAudioRecorder);
  await stopRecorder(recorder);

  if (window.__gmrAudioMixer) {
    // Final audio_health report; Meet's own remote tracks are left untouched.
    window.__gmrAudioMixer.stop();
    window.__gmrAudioMixer = null;
  }

  if (window.__gmrRecorderStream) {
    window.__gmrRecorderStream.getTracks().forEach((track) => track.stop());
  }
  if (window.__gmrCaptureStreams) {
    window.__gmrCaptureStreams.forEach((stream) => stream.getTracks().forEach((track) => track.stop()));
  }
  if (window.__gmrTimelineInterval) {
    window.clearInterval(window.__gmrTimelineInterval);
    window.__gmrTimelineInterval = null;
  }
  if (window.__gmrActiveSpeakerInterval) {
    window.clearInterval(window.__gmrActiveSpeakerInterval);
    window.__gmrActiveSpeakerInterval = null;
  }
  if (window.__gmrTimelineCollector && typeof window.__gmrSendTimelineEvents === 'function') {
    window.__gmrSendTimelineEvents(window.__gmrTimelineCollector.collect());
  }
  if (window.__gmrAudioContext && window.__gmrAudioContext.state !== 'closed') {
    await window.__gmrAudioContext.close();
  }
  await Promise.allSettled(window.__gmrUploads || []);
  if (typeof window.__gmrRecordChunk === 'function') {
    window.__gmrRecordChunk(JSON.stringify({ kind: 'done' }));
  }
  return { status: 'stopped', errors: window.__gmrUploadErrors || [] };
}
