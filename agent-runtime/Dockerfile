FROM python:3.11-slim

WORKDIR /app

COPY pyproject.toml ./
RUN pip install --no-cache-dir .

COPY agent_runtime/ agent_runtime/

ENTRYPOINT ["python", "-m", "agent_runtime"]
