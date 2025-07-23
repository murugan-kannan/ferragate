# Build stage
FROM rust:1.82-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the application in release mode
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install necessary runtime dependencies and create user
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r ferragate && useradd -r -g ferragate -m ferragate

# Create application directory and logs directory
RUN mkdir -p /app/logs && \
    chown -R ferragate:ferragate /app

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/ferragate /usr/local/bin/ferragate

# Copy default configuration
COPY gateway.toml /app/gateway.toml

# Set ownership and permissions
RUN chown ferragate:ferragate /usr/local/bin/ferragate && \
    chmod +x /usr/local/bin/ferragate && \
    chown ferragate:ferragate /app/gateway.toml

# Switch to non-root user
USER ferragate

# Set working directory
WORKDIR /app

# Set environment variables
ENV RUST_LOG=info
ENV LOG_DIR=/app/logs

# Expose the port (default gateway port)
EXPOSE 3000

# Health check using the new default port
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/health/live || exit 1

# Run the application with configuration file
CMD ["ferragate", "start", "--config", "gateway.toml"]
