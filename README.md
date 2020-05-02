# Codewars Bot (for Slack)

![CI](https://github.com/dnaka91/codewars-bot/workflows/CI/badge.svg)

A Slack bot to report [codewars](https://codewars.com) statistics, written in Rust 🦀.

## Setup

The application currently expects settings to be provided through environment variables provided by
an `.env` file. The required variables are as follows:

| Variable    | Description                                      |
| ----------- | ------------------------------------------------ |
| SIGNING_KEY | Key to verify the HTTP calls come from Slack     |
| WEBHOOK_URL | Webhook to send messages to a Slack team channel |

## Build

Have the latest `rust` toolchain and `cargo` installed and run:

```shell
cargo build
```

### Docker

The project contains a `Dockerfile` so you can package the bot as an independent image.

```shell
docker build -t codewars-bot .
```

## Running

Well simply execute the compiled binary, run directly through cargo (`cargo run`) or run the
previously built Docker image.

## License

This project is licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE) (or <http://www.apache.org/licenses/LICENSE-2.0>)
- [MIT License](LICENSE-MIT) (or <http://opensource.org/licenses/MIT>)

at your option.
