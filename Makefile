.PHONY: up down test lint typecheck migrate bootstrap

up:
	docker compose up --build

down:
	docker compose down

migrate:
	uv run alembic upgrade head

test:
	uv run pytest -q

lint:
	uv run ruff check .
	cd apps/web && bun run lint

typecheck:
	uv run mypy apps/api/src apps/worker/src packages/meetops_common/src || true

bootstrap:
	uv run meetops-cli bot profile-bootstrap
