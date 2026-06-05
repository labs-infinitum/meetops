import { hasStringProperty } from "@/lib/type-guards"

const SESSION_INVALID_CODE = "session_not_found"

export const SESSION_EXPIRED_MESSAGE =
  "Your session has expired. Please sign in again."

let sessionInvalidHandler: (() => void) | null = null

export function registerSessionInvalidHandler(handler: () => void) {
  sessionInvalidHandler = handler
}

function notifySessionInvalid() {
  sessionInvalidHandler?.()
}

export function isSessionExpired(expiresAt: number | undefined): boolean {
  if (expiresAt === undefined || !Number.isFinite(expiresAt) || expiresAt <= 0) {
    return true
  }
  return Math.floor(Date.now() / 1000) >= expiresAt
}

export function isSessionInvalidError(error: unknown): boolean {
  return hasStringProperty(error, "code") && error.code === SESSION_INVALID_CODE
}

function guardApiError(error: unknown): never {
  if (isSessionInvalidError(error)) {
    notifySessionInvalid()
  }
  throw error
}

export async function withAuthGuard<T>(fn: () => Promise<T>): Promise<T> {
  return fn().catch((error) => guardApiError(error))
}
