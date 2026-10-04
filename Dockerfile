FROM debian:bookworm-slim

EXPOSE 8080

# setup users
RUN groupadd -g 1000 appgroup && \
    useradd -u 1000 -g appgroup -m -s /bin/sh appuser

WORKDIR /app

COPY --chown=appuser:appgroup target/dx/mutter/release/web/public ./public
COPY --chown=appuser:appgroup target/dx/mutter/release/web/server ./

ENTRYPOINT ["/app/server"]
