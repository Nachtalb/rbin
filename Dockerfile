# Stage 1: Build the statically linked application using the Rust MUSL target
FROM rust:1-slim-bookworm as builder

# Install MUSL target and build tools (musl-gcc)
RUN rustup target add x86_64-unknown-linux-musl && \
    apt-get update && \
    apt-get install -y musl-tools && \
    rm -rf /var/lib/apt/lists/*

# Set the working directory in the container
WORKDIR /usr/src/rbin

# Copy the Cargo manifest files
COPY Cargo.toml Cargo.lock ./

# Build dependencies first to leverage Docker cache for MUSL target
# Create a dummy src/main.rs to build only dependencies
# Note: Output is in target/x86_64-unknown-linux-musl/release
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl && \
    rm -rf src target/x86_64-unknown-linux-musl/release/deps/rbin* # Clean dummy deps

# Copy the actual source code
COPY src ./src

# Build the release binary for MUSL target
# Ensure the target name matches your Cargo.toml (e.g., rbin)
# Remove the dummy binary artifact first
RUN rm ./target/x86_64-unknown-linux-musl/release/rbin && \
    cargo build --release --target x86_64-unknown-linux-musl

# Stage 2: Create the final minimal image using scratch
# Since we built a static binary with MUSL, we don't need libc or an OS
FROM scratch

# Set the working directory
WORKDIR /app

# Copy the compiled static binary from the builder stage
# Cargo names the binary 'rbin' (from Cargo.toml), but we copy it as 'rbin'
COPY --from=builder /usr/src/rbin/target/x86_64-unknown-linux-musl/release/rbin ./rbin

# Create the default paste directory. The application needs permissions to write here.
# In scratch, there's no user concept, files are owned by root.
# Mounting a volume at runtime is recommended for persistent storage.
# RUN mkdir pastes # App creates this on demand, so maybe not needed here.

# Expose the default port the application listens on
# The actual port can be changed via RBIN_PORT env var at runtime
EXPOSE 3000

# Command to run the application
# The binary is now named 'rbin' in this stage
CMD ["/app/rbin"]


