import { GrpcWebFetchTransport } from "@protobuf-ts/grpcweb-transport"
import { AuthServiceClient } from "@/gen/notetaker/v1/auth.client"

const grpcBaseUrl = import.meta.env.VITE_GRPC_BASE_URL ?? ""

let authClient: AuthServiceClient | null = null

export function getAuthClient(): AuthServiceClient {
  if (!authClient) {
    const transport = new GrpcWebFetchTransport({
      baseUrl: grpcBaseUrl,
      format: "binary",
    })
    authClient = new AuthServiceClient(transport)
  }
  return authClient
}
