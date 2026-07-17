ARG PYTHON_IMAGE=python:3.12-slim-bookworm
FROM ${PYTHON_IMAGE}

ENV DEBIAN_FRONTEND=noninteractive \
    PYTHONUNBUFFERED=1 \
    MEETOPS_DATA_DIR=/data \
    DISPLAY=:99 \
    PATCHRIGHT_BROWSERS_PATH=/opt/ms-playwright \
    PLAYWRIGHT_BROWSERS_PATH=/opt/ms-playwright

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl build-essential libpq-dev \
    xvfb openbox x11vnc novnc websockify xdotool ffmpeg \
    fonts-liberation fonts-noto-color-emoji \
    libnss3 libnspr4 libatk1.0-0 libatk-bridge2.0-0 libcups2 libdrm2 \
    libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 libxrandr2 \
    libgbm1 libasound2 libpango-1.0-0 libcairo2 \
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
    && mkdir -p /opt/ms-playwright /data \
    && /app/.venv/bin/patchright install chromium \
    && chown -R meetops:meetops /app /data /opt/ms-playwright

ENV PATH="/app/.venv/bin:$PATH"

USER meetops
ENTRYPOINT ["/app/scripts/entrypoint-worker.sh"]
