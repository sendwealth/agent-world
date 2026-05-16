FROM python:3.11-slim

WORKDIR /app

COPY pyproject.toml ./
COPY agent_runtime/ agent_runtime/
RUN pip install --no-cache-dir .

ENTRYPOINT ["python", "-m", "agent_runtime"]
