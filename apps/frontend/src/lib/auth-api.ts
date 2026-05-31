import type { AuthSession, User } from "@/types"
import { getAuthClient } from "@/lib/grpc-client"
import { formatGrpcApiError, GrpcApiError, isGrpcErrorVariant, isGrpcSuccessVariant } from "@/lib/api-errors"
import { SESSION_EXPIRED_MESSAGE } from "@/lib/session-guard"
import type { AuthResponse } from "@ai-meeting-notetaker/proto/notetaker/v1/auth"

export class AuthApiError extends GrpcApiError {
  constructor(code: string, message: string) {
    super(code, message, "AuthApiError")
  }
}

function mapUser(user: {
  id: string
  name: string
  email: string
  username: string
}): User {
  return {
    id: user.id,
    name: user.name,
    email: user.email,
    username: user.username,
  }
}

function parseAuthResponse(response: AuthResponse): AuthSession {
  if (isGrpcSuccessVariant(response.result)) {
    const success = response.result.success
    if (!success.user) {
      throw new AuthApiError("internal_error", "Missing user in auth response.")
    }
    return {
      user: mapUser(success.user),
      token: success.token,
      expiresAt: Number(success.expiresAt),
    }
  }

  if (isGrpcErrorVariant(response.result)) {
    throw new AuthApiError(response.result.error.code, response.result.error.message)
  }

  throw new AuthApiError("internal_error", "Empty auth response.")
}

export async function apiSignUp(
  name: string,
  email: string,
  username: string,
  password: string,
): Promise<AuthSession> {
  const client = getAuthClient()
  const { response } = await client.signUp({ name, email, username, password })
  return parseAuthResponse(response)
}

export async function apiLogin(username: string, password: string): Promise<AuthSession> {
  const client = getAuthClient()
  const { response } = await client.login({ username, password })
  return parseAuthResponse(response)
}

export async function apiLogout(token: string): Promise<void> {
  const client = getAuthClient()
  await client.logout({ token })
}

export async function apiGetSession(token: string): Promise<AuthSession> {
  const client = getAuthClient()
  const { response } = await client.getSession({ token })
  return parseAuthResponse(response)
}

export function authErrorMessage(error: unknown): string {
  return formatGrpcApiError(
    error,
    (code, message) => {
      switch (code) {
        case "username_taken":
          return "That username is already taken."
        case "email_taken":
          return "An account with this email already exists."
        case "invalid_credentials":
          return "Invalid username or password."
        case "session_not_found":
          return SESSION_EXPIRED_MESSAGE
        case "invalid_input":
          return message
        default:
          return null
      }
    },
    "Authentication failed.",
  )
}
