# Multi-stage build for Rust backend
FROM rust:1.75 as rust-builder

WORKDIR /app

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY src src/

# Build the Rust applications
RUN cargo build --release --bins

# Node.js build stage for frontend
FROM node:18-alpine as frontend-builder

WORKDIR /app/frontend

# Copy package files for dependency caching
COPY frontend/package*.json ./
RUN npm ci --only=production

# Copy source code and build
COPY frontend/ ./
RUN npm run build

# Final runtime image
FROM ubuntu:22.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN adduser --system --disabled-password --no-create-home --home /app doomsday

WORKDIR /app

# Copy built binaries from Rust builder
COPY --from=rust-builder /app/target/release/doomsday-server ./
COPY --from=rust-builder /app/target/release/doomsday-cli ./

# Copy built frontend from Node builder
COPY --from=frontend-builder /app/frontend/.next/standalone ./frontend/
COPY --from=frontend-builder /app/frontend/.next/static ./frontend/.next/static/
COPY --from=frontend-builder /app/frontend/public ./frontend/public/

# Copy configuration
COPY ddayconfig.yml ./

# Set ownership
RUN chown -R doomsday:doomsday /app

USER doomsday

# Expose ports
EXPOSE 8111 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
  CMD curl -f http://localhost:8111/v1/info || exit 1

# Default command runs the server
CMD ["./doomsday-server"]