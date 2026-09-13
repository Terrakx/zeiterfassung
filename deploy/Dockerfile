# Multi-Stage-Build: Frontend (Node) → Backend (Rust) → Laufzeit (Debian + TeX Live)
# Build aus dem Repository-Wurzelverzeichnis:  docker build -f deploy/Dockerfile -t timecard .

FROM node:22-bookworm-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json* ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.93-bookworm AS server
WORKDIR /src
COPY server/ ./server/
COPY latex/ ./latex/
COPY --from=web /src/web/build ./web/build
WORKDIR /src/server
RUN cargo build --release && strip target/release/timecard-server

FROM debian:bookworm-slim AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        texlive-luatex texlive-latex-base texlive-latex-recommended texlive-latex-extra \
        texlive-fonts-recommended texlive-lang-german latexmk fonts-lmodern fonts-ibm-plex \
        ca-certificates tzdata sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && luaotfload-tool --update >/dev/null 2>&1 || true
ENV TZ=Europe/Vienna
RUN useradd -r -u 10001 -d /data timecard && mkdir -p /data && chown timecard:timecard /data
COPY --from=server /src/server/target/release/timecard-server /usr/local/bin/timecard-server
USER timecard
WORKDIR /data
ENV TIMECARD_DATA_DIR=/data TIMECARD_BIND=0.0.0.0:8080 RUST_LOG=info
EXPOSE 8080
VOLUME ["/data"]
HEALTHCHECK --interval=30s --timeout=5s CMD ["/bin/sh", "-c", "exec 3<>/dev/tcp/127.0.0.1/8080 && printf 'GET /api/health HTTP/1.0\\r\\n\\r\\n' >&3 && grep -q ok <&3"]
ENTRYPOINT ["/usr/local/bin/timecard-server"]
