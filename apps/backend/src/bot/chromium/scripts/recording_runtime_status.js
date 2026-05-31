function() {
  const base = window.__gmrRecordingStatus || { status: 'not_installed' };
  return {
    ...base,
    chunks: window.__gmrChunkCount || 0,
    uploadedBytes: window.__gmrUploadedBytes || 0,
    acceptedChunks: window.__gmrAcceptedChunks || 0,
    uploadErrors: window.__gmrUploadErrors || [],
    bindingAvailable: !!window.__gmrChunkBindingAvailable,
    streamInfo: window.__gmrStreamInfo || null,
    lastParticipantCount: typeof window.__gmrLastParticipantCount === 'number' ? window.__gmrLastParticipantCount : null,
    lastParticipantSource: window.__gmrLastParticipantSource || null
  };
}
