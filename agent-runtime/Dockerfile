FROM python:3.11-slim

WORKDIR /app

# Configurable PyPI mirror via build arg. Defaults to official pypi.org.
ARG PYPI_MIRROR=pypi.org

# Configure PyPI mirror via environment variables.
# Default is pypi.org (official); docker-compose passes Tsinghua mirror for CN builds.
ENV PIP_INDEX_URL=https://${PYPI_MIRROR}/simple/
ENV PIP_TRUSTED_HOST=${PYPI_MIRROR}
ENV PIP_NO_CACHE_DIR=1

RUN pip install --no-cache-dir --upgrade pip
RUN pip install --no-cache-dir hatchling

COPY pyproject.toml ./
COPY agent_runtime/ agent_runtime/
RUN pip install --no-cache-dir .

RUN groupadd --system appuser && useradd --system --gid appuser appuser \
    && chown -R appuser:appuser /app
USER appuser

ENTRYPOINT ["python", "-m", "agent_runtime"]
