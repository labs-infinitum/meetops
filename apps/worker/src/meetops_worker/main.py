"""Worker entrypoint."""

from __future__ import annotations

from meetops_api.workers.app import run, run_worker

__all__ = ["run", "run_worker"]


def main() -> None:
    run()


if __name__ == "__main__":
    main()