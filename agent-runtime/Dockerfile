FROM python:3.11-slim

WORKDIR /app

RUN pip install --no-cache-dir --upgrade pip hatchling

COPY pyproject.toml ./
COPY agent_runtime/ agent_runtime/
RUN pip install --no-cache-dir .

RUN groupadd --system appuser && useradd --system --gid appuser appuser
USER appuser

ENTRYPOINT ["python", "-m", "agent_runtime"]
