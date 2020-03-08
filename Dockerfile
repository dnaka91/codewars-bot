# syntax = docker/dockerfile:experimental
FROM clux/muslrust:stable as builder

COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/volume/target \
    cargo install --path .

FROM alpine:3.11

WORKDIR /data

RUN apk add --no-cache ca-certificates tzdata

COPY --from=builder /root/.cargo/bin/codewars-bot /app/

EXPOSE 8080

ENTRYPOINT ["/app/codewars-bot"]
