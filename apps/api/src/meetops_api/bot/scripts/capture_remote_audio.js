/*
 * Installed before Meet's own scripts (context.add_init_script, page main world).
 * Meet plays participants' audio without DOM <audio> elements, so the recorder's mixer (which
 * scans audio/video elements' srcObject) found nothing. This hands every remote audio track
 * Meet receives to a hidden, muted <audio> element, which the mixer then picks up. Muted
 * playback still matters: Chrome only feeds remote WebRTC audio into Web Audio while the
 * track is attached to a playing media element.
 */
function() {
  if (window.__gmrRtcHooked) return;  // init scripts can run more than once per document
  window.__gmrRtcHooked = true;
  const Native = window.RTCPeerConnection;
  if (typeof Native !== 'function') return;
  const seen = new Set();

  const attach = (track) => {
    if (!track || track.kind !== 'audio' || seen.has(track.id)) return;
    seen.add(track.id);
    const el = document.createElement('audio');
    el.setAttribute('data-gmr-remote-audio', track.id);
    el.muted = true;
    el.style.display = 'none';
    el.srcObject = new MediaStream([track]);
    const mount = () => {
      (document.body || document.documentElement).appendChild(el);
      el.play().catch(() => {});
    };
    if (document.body) mount();
    else document.addEventListener('DOMContentLoaded', mount, { once: true });
    track.addEventListener('ended', () => {
      el.remove();
      seen.delete(track.id);
    }, { once: true });
  };

  class RTCPeerConnection extends Native {
    constructor(...args) {
      super(...args);
      this.addEventListener('track', (event) => attach(event.track));
    }
  }
  window.RTCPeerConnection = RTCPeerConnection;
  if ('webkitRTCPeerConnection' in window) window.webkitRTCPeerConnection = RTCPeerConnection;
}
