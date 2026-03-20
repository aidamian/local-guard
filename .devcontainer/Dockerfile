FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bubblewrap \
        mingw-w64 \
        nodejs \
        npm \
    && rm -rf /var/lib/apt/lists/*
