# Build stage
FROM rust:1.80-slim AS builder

WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the application
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

# Set ownership and permissions
RUN chown ferragate:ferragate /usr/local/bin/ferragate && \
    chmod +x /usr/local/bin/ferragate

# Switch to non-root user
USER ferragate

# Set working directory
WORKDIR /app

# Expose the port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:3000/health/live || exit 1

# Run the application
CMD ["ferragate"]
