FROM python:3.11-slim

ENV PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    VENV_PATH=/opt/venv

WORKDIR /app

RUN apt-get update && apt-get upgrade -y && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /usr/sbin/nologin app && \
    mkdir -p /data/models && \
    chown -R app:app /app /data

COPY --chown=app:app requirements.txt .
RUN python -m venv "${VENV_PATH}" && \
    "${VENV_PATH}/bin/pip" install --upgrade pip setuptools wheel && \
    "${VENV_PATH}/bin/pip" install -r requirements.txt

COPY --chown=app:app app.py .

USER app

# Pre-download the embedding model so first request isn't slow.
RUN "${VENV_PATH}/bin/python" -c "from sentence_transformers import SentenceTransformer; SentenceTransformer('all-MiniLM-L6-v2', cache_folder='/data/models')" || true

EXPOSE 3200

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD ["sh", "-c", "${VENV_PATH}/bin/python -c \"import urllib.request; urllib.request.urlopen('http://127.0.0.1:3200/health', timeout=5)\""]

CMD ["/opt/venv/bin/python", "-m", "uvicorn", "app:app", "--host", "0.0.0.0", "--port", "3200"]
