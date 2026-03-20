FROM node:20-slim AS builder

WORKDIR /app

# Install git only for dependency resolution.
RUN apt-get update && apt-get upgrade -y && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Force git to use HTTPS instead of SSH for GitHub.
RUN git config --global url."https://github.com/".insteadOf "ssh://git@github.com/" && \
    git config --global url."https://github.com/".insteadOf "git@github.com:"

COPY package.json package-lock.json ./
RUN npm ci --omit=dev && \
    npm cache clean --force && \
    rm -rf /root/.npm /root/.gitconfig /tmp/*

COPY index.js ./

FROM node:20-slim

ENV NODE_ENV=production \
    BRIDGE_PORT=8999 \
    BRIDGE_HOST=127.0.0.1 \
    AUTH_DIR=/data/auth

WORKDIR /app

RUN groupadd -r bridge && \
    useradd -r -g bridge -m -d /home/bridge -s /usr/sbin/nologin bridge && \
    mkdir -p /data/auth && \
    chown -R bridge:bridge /data /app

COPY --from=builder --chown=bridge:bridge /app/package.json ./package.json
COPY --from=builder --chown=bridge:bridge /app/node_modules ./node_modules
COPY --from=builder --chown=bridge:bridge /app/index.js ./index.js

USER bridge

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:8999/health').then(r => r.ok ? process.exit(0) : process.exit(1)).catch(() => process.exit(1))"

CMD ["node", "index.js"]
