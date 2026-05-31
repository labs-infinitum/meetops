import type { AuthSession, User } from "@/types"

import { hasStringProperty } from "@/lib/type-guards"

function isUser(value: unknown): value is User {
  return (
    typeof value === "object" &&
    value !== null &&
    hasStringProperty(value, "id") &&
    hasStringProperty(value, "name") &&
    hasStringProperty(value, "email") &&
    hasStringProperty(value, "username") &&
    (!("avatarUrl" in value) ||
      (value as { avatarUrl?: unknown }).avatarUrl === undefined ||
      typeof (value as { avatarUrl?: unknown }).avatarUrl === "string")
  )
}

export function parseStoredAuthSession(raw: string): AuthSession | null {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }

  if (typeof parsed !== "object" || parsed === null) {
    return null
  }

  if (!hasStringProperty(parsed, "token")) {
    return null
  }

  const userValue = (parsed as Record<string, unknown>).user
  if (!isUser(userValue)) {
    return null
  }

  const expiresAt = (parsed as Record<string, unknown>).expiresAt
  if (
    expiresAt !== undefined &&
    (typeof expiresAt !== "number" || !Number.isFinite(expiresAt))
  ) {
    return null
  }

  return {
    user: userValue,
    token: parsed.token,
    expiresAt,
  }
}
