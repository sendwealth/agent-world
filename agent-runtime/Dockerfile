FROM python:3.11-slim

WORKDIR /app

# Use Chinese PyPI mirror for reliable downloads
RUN pip install --no-cache-dir --upgrade pip \
    && pip config set global.index-url https://pypi.tuna.tsinghua.edu.cn/simple \
    && pip config set global.trusted-host pypi.tuna.tsinghua.edu.cn

RUN pip install --no-cache-dir hatchling

COPY pyproject.toml ./
COPY agent_runtime/ agent_runtime/
RUN pip install --no-cache-dir .

RUN groupadd --system appuser && useradd --system --gid appuser appuser \
    && chown -R appuser:appuser /app
USER appuser

ENTRYPOINT ["python", "-m", "agent_runtime"]
