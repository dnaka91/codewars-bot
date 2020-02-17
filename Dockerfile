FROM clux/muslrust:stable as builder

COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN cargo install --path .

FROM alpine:3.11

WORKDIR /data

RUN apk add --no-cache ca-certificates tzdata

COPY --from=builder /root/.cargo/bin/codewars-bot /app/

ENTRYPOINT ["/app/codewars-bot"]
