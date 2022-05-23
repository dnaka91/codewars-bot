FROM rust:1.60-alpine as builder

WORKDIR /volume

RUN apk add --no-cache musl-dev=~1.2

COPY assets/ assets/
COPY src/ src/
COPY Cargo.lock Cargo.toml ./

RUN cargo build --release && \
    strip --strip-all target/release/codewars-bot

FROM alpine:3.16.0 as newuser

RUN echo "codewars-bot:x:1000:" > /tmp/group && \
    echo "codewars-bot:x:1000:1000::/dev/null:/sbin/nologin" > /tmp/passwd

FROM scratch

COPY --from=builder /volume/target/release/codewars-bot /bin/
COPY --from=newuser /tmp/group /tmp/passwd /etc/

EXPOSE 8080
STOPSIGNAL SIGINT
USER codewars-bot

ENTRYPOINT ["/bin/codewars-bot"]
