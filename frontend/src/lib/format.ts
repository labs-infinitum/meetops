const dateFormatter = new Intl.DateTimeFormat(undefined, {
  weekday: "short",
  month: "short",
  day: "numeric",
})

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
})

const dateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  hour: "numeric",
  minute: "2-digit",
})

export function formatDate(unix: number) {
  return dateFormatter.format(new Date(unix * 1000))
}

export function formatTime(unix: number) {
  return timeFormatter.format(new Date(unix * 1000))
}

export function formatDateTime(unix: number) {
  return dateTimeFormatter.format(new Date(unix * 1000))
}

export function formatDuration(secs: number) {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

export function formatRelative(unix: number) {
  const diff = unix - Math.floor(Date.now() / 1000)
  const abs = Math.abs(diff)
  if (abs < 60) return diff >= 0 ? "in a moment" : "just now"
  if (abs < 3600) {
    const mins = Math.round(abs / 60)
    return diff >= 0 ? `in ${mins}m` : `${mins}m ago`
  }
  if (abs < 86400) {
    const hrs = Math.round(abs / 3600)
    return diff >= 0 ? `in ${hrs}h` : `${hrs}h ago`
  }
  const days = Math.round(abs / 86400)
  return diff >= 0 ? `in ${days}d` : `${days}d ago`
}

export function meetingTimeRange(startAt: number, endAt: number) {
  return `${formatTime(startAt)} – ${formatTime(endAt)}`
}
