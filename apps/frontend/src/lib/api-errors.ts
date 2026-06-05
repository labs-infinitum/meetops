export class GrpcApiError extends Error {
  code: string

  constructor(code: string, message: string, name: string) {
    super(message)
    this.code = code
    this.name = name
  }
}

const GRPC_CONNECTION_ERROR =
  "Cannot reach the server. Is the backend running?"

function isNetworkFetchError(error: Error): boolean {
  return (
    error.message.includes("Failed to fetch") ||
    error.message.includes("NetworkError")
  )
}

function messageFromUnknownError(error: unknown, fallback: string): string {
  if (error instanceof Error) {
    if (isNetworkFetchError(error)) {
      return GRPC_CONNECTION_ERROR
    }
    return error.message
  }
  return fallback
}

export function formatGrpcApiError(
  error: unknown,
  mapCode: (code: string, message: string) => string | null,
  fallback: string,
): string {
  if (error instanceof GrpcApiError) {
    const mapped = mapCode(error.code, error.message)
    if (mapped !== null) {
      return mapped
    }
    return error.message || fallback
  }
  return messageFromUnknownError(error, fallback)
}

export type GrpcOneofResult<S, E> =
  | { oneofKind: "success"; success: S }
  | { oneofKind: "error"; error: E }
  | { oneofKind?: undefined }

function isGrpcSuccessVariant<S, E>(
  result: GrpcOneofResult<S, E>,
): result is { oneofKind: "success"; success: S } {
  return result.oneofKind === "success"
}

function isGrpcErrorVariant<S, E>(
  result: GrpcOneofResult<S, E>,
): result is { oneofKind: "error"; error: E } {
  return result.oneofKind === "error"
}

export function unwrapGrpcOneof<S, E, T>(
  result: GrpcOneofResult<S, E> | undefined,
  handlers: {
    success: (value: S) => T
    error: (err: E) => Error
    emptyMessage: string
    EmptyError: new (code: string, message: string) => Error
  },
): T {
  if (!result) {
    throw new handlers.EmptyError("internal_error", handlers.emptyMessage)
  }
  if (isGrpcSuccessVariant(result)) {
    return handlers.success(result.success)
  }
  if (isGrpcErrorVariant(result)) {
    throw handlers.error(result.error)
  }
  throw new handlers.EmptyError("internal_error", handlers.emptyMessage)
}
