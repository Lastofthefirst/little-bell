# Build stage
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy dependency files
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm src/main.rs

# Copy source code
COPY src ./src
COPY templates ./templates

# Build the application
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1001 appuser

# Create data directory
RUN mkdir -p /app/data && chown appuser:appuser /app/data

# Copy binary from builder
COPY --from=builder /app/target/release/little-bell /app/little-bell

# Set ownership
RUN chown appuser:appuser /app/little-bell

# Switch to app user
USER appuser

# Set working directory
WORKDIR /app

# Default environment variables
ENV PORT=3000
ENV DATABASE_URL=sqlite:data/tracking.db
ENV BASE_URL=http://localhost:3000
ENV RUST_LOG=little_bell=info

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run the application
CMD ["./little-bell"]