import type { AuthSession, User } from "@/types"
import { getAuthClient } from "@/lib/grpc-client"
import { formatGrpcApiError, GrpcApiError, unwrapGrpcOneof } from "@/lib/api-errors"
import { SESSION_EXPIRED_MESSAGE } from "@/lib/session-guard"
import type {
  AuthMessageResponse,
  AuthResponse,
} from "@ai-meeting-notetaker/proto/notetaker/v1/auth"

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
  emailVerified: boolean
}): User {
  return {
    id: user.id,
    name: user.name,
    email: user.email,
    username: user.username,
    emailVerified: user.emailVerified,
  }
}

function parseAuthResponse(response: AuthResponse): AuthSession {
  return unwrapGrpcOneof(response.result, {
    success: (success) => {
      if (!success.user) {
        throw new AuthApiError("internal_error", "Missing user in auth response.")
      }
      if (success.emailVerificationRequired || !success.token) {
        throw new AuthApiError(
          "email_verification_required",
          "Please verify your email address before signing in.",
        )
      }
      return {
        user: mapUser(success.user),
        token: success.token,
        expiresAt: Number(success.expiresAt),
      }
    },
    error: (error) => new AuthApiError(error.code, error.message),
    emptyMessage: "Empty auth response.",
    EmptyError: AuthApiError,
  })
}

function parseAuthMessageResponse(response: AuthMessageResponse): string {
  return unwrapGrpcOneof(response.result, {
    success: (success) => success.message,
    error: (error) => new AuthApiError(error.code, error.message),
    emptyMessage: "Empty auth response.",
    EmptyError: AuthApiError,
  })
}

export type SignUpResult =
  | { kind: "session"; session: AuthSession }
  | { kind: "verification_required"; user: User }

export async function apiSignUp(
  name: string,
  email: string,
  username: string,
  password: string,
): Promise<SignUpResult> {
  const client = getAuthClient()
  const { response } = await client.signUp({ name, email, username, password })
  return unwrapGrpcOneof(response.result, {
    success: (success) => {
      if (!success.user) {
        throw new AuthApiError("internal_error", "Missing user in auth response.")
      }
      if (success.emailVerificationRequired || !success.token) {
        return {
          kind: "verification_required" as const,
          user: mapUser(success.user),
        }
      }
      return {
        kind: "session" as const,
        session: {
          user: mapUser(success.user),
          token: success.token,
          expiresAt: Number(success.expiresAt),
        },
      }
    },
    error: (error) => new AuthApiError(error.code, error.message),
    emptyMessage: "Empty auth response.",
    EmptyError: AuthApiError,
  })
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

export async function apiRequestPasswordReset(email: string): Promise<string> {
  const client = getAuthClient()
  const { response } = await client.requestPasswordReset({ email })
  return parseAuthMessageResponse(response)
}

export async function apiResetPassword(token: string, newPassword: string): Promise<string> {
  const client = getAuthClient()
  const { response } = await client.resetPassword({ token, newPassword })
  return parseAuthMessageResponse(response)
}

export async function apiVerifyEmail(token: string): Promise<AuthSession> {
  const client = getAuthClient()
  const { response } = await client.verifyEmail({ token })
  return parseAuthResponse(response)
}

export async function apiResendVerificationEmail(email: string): Promise<string> {
  const client = getAuthClient()
  const { response } = await client.resendVerificationEmail({ email })
  return parseAuthMessageResponse(response)
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
        case "email_not_verified":
          return "Please verify your email address before signing in."
        case "email_already_verified":
          return "This email address is already verified."
        case "invalid_token":
          return "This link is invalid or has expired."
        case "email_send_failed":
          return "We couldn't send the email. Please try again later."
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
