FROM node:22.20.0-bookworm-slim@sha256:b21fe589dfbe5cc39365d0544b9be3f1f33f55f3c86c87a76ff65a02f8f5848e AS node

FROM docker:29.6.1-dind@sha256:66d292e5c26bd33a6f6f61cacb880de2186339a524ecba1ce098dbbaceed6515 AS docker

FROM ghcr.io/astral-sh/uv:0.11.32-python3.12-trixie-slim@sha256:519357e414a4240af8b3ac657466c20f9d6041b5a3ce999d96fa9d576ef7fd29

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    docker-cli \
    docker.io \
    git \
    libstdc++6 \
  && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=docker /usr/local/libexec/docker/cli-plugins/docker-compose /usr/local/lib/docker/cli-plugins/docker-compose

RUN chmod 0555 /usr/local/lib/docker/cli-plugins/docker-compose \
  && node --version \
  && uv --version \
  && docker compose version

ENV UV_CACHE_DIR=/root/.cache/uv \
  UV_PYTHON=3.12

WORKDIR /benchmark

COPY project.json ./project.json
COPY agents ./agents
COPY dist/mosaic-bench-acp.mjs ./dist/mosaic-bench-acp.mjs
COPY dist/mosaic-bench-host.mjs ./dist/mosaic-bench-host.mjs
COPY docker-entrypoint.sh /usr/local/bin/mosaic-benchmark-entrypoint

RUN chmod 0555 /usr/local/bin/mosaic-benchmark-entrypoint

ENTRYPOINT ["mosaic-benchmark-entrypoint"]
CMD ["help"]
