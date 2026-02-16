FROM node:20-slim

WORKDIR /app

# Install git + openssh (required by Baileys npm dependency) and clean up
RUN apt-get update && apt-get install -y --no-install-recommends git openssh-client && rm -rf /var/lib/apt/lists/*

# Force git to use HTTPS instead of SSH for GitHub
RUN git config --global url."https://github.com/".insteadOf "ssh://git@github.com/" && \
    git config --global url."https://github.com/".insteadOf "git@github.com:"

# Security: run as non-root
RUN groupadd -r bridge && useradd -r -g bridge -m bridge

COPY package.json ./
RUN npm install --omit=dev && npm cache clean --force

COPY index.js ./

# Auth state persistence
RUN mkdir -p /data/auth && chown -R bridge:bridge /data /app

USER bridge

ENV BRIDGE_PORT=8999
ENV BRIDGE_HOST=127.0.0.1
ENV AUTH_DIR=/data/auth

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:8999/health').then(r => r.ok ? process.exit(0) : process.exit(1)).catch(() => process.exit(1))"

CMD ["node", "index.js"]
