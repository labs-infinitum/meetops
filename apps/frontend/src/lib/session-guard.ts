const SESSION_INVALID_CODE = "session_not_found"

let sessionInvalidHandler: (() => void) | null = null

export function registerSessionInvalidHandler(handler: () => void) {
  sessionInvalidHandler = handler
}

export function notifySessionInvalid() {
  sessionInvalidHandler?.()
}

export function isSessionExpired(expiresAt: number | undefined): boolean {
  if (expiresAt === undefined || !Number.isFinite(expiresAt) || expiresAt <= 0) {
    return true
  }
  return Math.floor(Date.now() / 1000) >= expiresAt
}

export function isSessionInvalidError(error: unknown): boolean {
  if (error && typeof error === "object" && "code" in error) {
    return (error as { code: string }).code === SESSION_INVALID_CODE
  }
  return false
}

export function guardApiError(error: unknown): never {
  if (isSessionInvalidError(error)) {
    notifySessionInvalid()
  }
  throw error
}

export async function withAuthGuard<T>(fn: () => Promise<T>): Promise<T> {
  try {
    return await fn()
  } catch (error) {
    guardApiError(error)
  }
}
