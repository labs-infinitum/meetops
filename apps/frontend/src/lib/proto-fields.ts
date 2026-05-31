/** Convert proto int64 to number; treat 0 as absent when `optional` is true. */
export function protoInt64(value: bigint, optional = false): number | undefined {
  const n = Number(value)
  if (optional && n === 0) return undefined
  return n
}

/** Map proto empty string to `undefined` for optional string fields. */
export function protoOptionalString(value: string): string | undefined {
  return value || undefined
}
