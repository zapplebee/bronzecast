FROM oven/bun:1.3.12-debian

ARG DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        chromium \
        ffmpeg \
        fonts-dejavu-core \
        libgl1-mesa-dri \
        libegl1 \
        libgbm1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/spikes/chromium-stickers
COPY spikes/chromium-stickers/package.json spikes/chromium-stickers/bun.lock ./
RUN bun install --frozen-lockfile
COPY spikes/chromium-stickers/ ./
COPY characters/mog/ /app/characters/mog/

ENV PORT=8091 \
    CHROME_PATH=/usr/bin/chromium \
    UDP_HOST=bronzecast-playout \
    UDP_PORT=5000

EXPOSE 8091
CMD ["bun", "src/server.ts"]
