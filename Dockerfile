# Stage 1: Build the application
FROM rust:1.78 as builder

# Install build dependencies
RUN apt-get -y update && \
    apt-get install -y clang && \
    apt-get autoremove -y; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/src/deoxys/

# Copy the source code into the container
COPY Cargo.toml Cargo.lock ./
COPY crates crates

# Build the application in release mode
RUN cargo build --release

# Stage 2: Create the final runtime image
FROM debian:bookworm

# Install runtime dependencies
RUN apt-get -y update && \
    apt-get install -y openssl ca-certificates tini &&\
    apt-get autoremove -y; \
    apt-get clean; \
    rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/local/bin

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/deoxys/target/release/deoxys .

# Create the data directory
RUN mkdir -p /var/lib/deoxys-testnet

# Expose the port
EXPOSE 9944

# Set the entrypoint to tini
ENTRYPOINT ["/usr/bin/tini", "--"]

# Set the command
CMD ["/usr/local/bin/deoxys", \
     "--rpc-external", \
     "--rpc-cors", "*", \
     "--base-path", "/var/lib/deoxys-testnet", \
     "--network", "test", \
     "--l1-endpoint", "https://eth-sepolia.g.alchemy.com/v2/WIUR5JUZXieEBkze6Xs3IOXWhsS840TX"]