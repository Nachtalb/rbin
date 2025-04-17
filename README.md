# rbin

## What is it?

rbin is a minimal pastebin service written in Rust. There is no interactive web
interface. Instead you can upload pasts with a simple HTTP POST request.

When you access the root URL (/) in a browser or via curl, it displays usage
help information as plain text.

## How to Use

You need curl or a similar tool to send data.

### Basic Usage

Pipe text directly using the form field name rbin:

```sh
echo "My important text snippet." | curl -F 'rbin=<-' http://<your-rbin-host>:<port>/
```

Pipe content from a file:

```sh
cat my_code.rs | curl -F 'rbin=<-' http://<your-rbin-host>:<port>/
```

### Example

```sh
echo "Hello from rbin!" | curl -F 'rbin=<-' http://localhost:3000/
http://localhost:3000/aBcDeF
```

You can then open http://localhost:3000/aBcDeF in your browser or use curl to
see the pasted text:

```sh
curl http://localhost:3000/aBcDeF
Hello from rbin!
```

## Configuration (Environment Variables)

You can configure rbin using the following environment variables:

- `RBIN_HOST`: The IP address to listen on (Default: `0.0.0.0`)
- `RBIN_PORT`: The port to listen on (Default: `3000`)
- `RBIN_PASTE_DIR`: The directory where paste files are stored (Default:
  `./pastes`)
- `RBIN_REQUEST_LOG_LEVEL`: Log level for HTTP requests (`tower_http`) if
  RUST_LOG is not set (Default: `debug`). Valid levels: `off`, `error`, `warn`,
  `info`, `trace`.
- `RUST_LOG`: Overrides all log levels if set (e.g., `info`,
  `rbin=debug,tower_http=warn`). Uses standard `tracing_subscriber::EnvFilter`
  syntax.

You can set these before running the application, for example:

```sh
export RBIN_PORT=8080
export RBIN_PASTE_DIR=/var/data/rbin_pastes
export RBIN_REQUEST_LOG_LEVEL=info
./target/release/rbin
```

Or place them in a `.env` file in the same directory as the executable.

## Building and Running

1. Make sure you have Rust installed (`rustup`).
2. Build the project: `cargo build --release`
3. Run the executable: `./target/release/rbin`

## Docker

Build the Docker image (see `Dockerfile`):

```sh
docker build -f Dockerfile.musl -t rbin .
```

Run the Docker container, mapping port 3000 and optionally mounting a volume for
persistent paste storage:

```sh
# Example: Run on port 8080, store pastes in ./my_pastes on the host
docker run -d
  -p 8080:3000
  -v ./my_pastes:/app/pastes
  -e RBIN_PORT=3000
  -e RBIN_PASTE_DIR=/app/pastes
  --name rbin
  rbin
```

(Adjust port mapping -p, volume mapping -v, and environment variables -e as
needed).
