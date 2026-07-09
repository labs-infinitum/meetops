# API

OpenAPI is served at `/docs` and `/redoc` when `MEETOPS_ENV` is not `production` (or `MEETOPS_ENABLE_OPENAPI=true`).

Base path: `/api/v1`

Auth: session cookie `meetops_session` (HttpOnly). Errors: `{ "error": { "code": "...", "message": "..." } }`.

All resource IDs in JSON are strings.
