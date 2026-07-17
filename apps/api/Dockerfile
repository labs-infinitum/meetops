ARG PYTHON_IMAGE=python:3.12-slim-bookworm
FROM ${PYTHON_IMAGE}

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl build-essential libpq-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv

WORKDIR /app
RUN useradd -m -u 1000 meetops

COPY pyproject.toml uv.lock* README.md ./
COPY packages/meetops_common packages/meetops_common
COPY apps/api apps/api
COPY apps/worker apps/worker
COPY alembic alembic
COPY alembic.ini ./
COPY scripts scripts
RUN chmod +x /app/scripts/*.sh

RUN uv sync --frozen --no-dev || uv sync --no-dev \
    && chown -R meetops:meetops /app

ENV PATH="/app/.venv/bin:$PATH" \
    MEETOPS_DATA_DIR=/data \
    PYTHONUNBUFFERED=1

USER meetops
EXPOSE 8000

HEALTHCHECK CMD curl -f http://localhost:8000/healthz || exit 1
ENTRYPOINT ["/app/scripts/entrypoint-api.sh"]
