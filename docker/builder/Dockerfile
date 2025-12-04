FROM rustlang/rust:nightly

# Install all build dependencies
RUN apt-get update && apt-get install -y \
    git \
    ca-certificates \
    openssl \
    libssl-dev \
    curl \
    gnupg \
    make \
    protobuf-compiler \
    unzip \
    build-essential \
    swi-prolog \
    libgmp-dev \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 20 and npm (needed for Elm/frontend)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && npm install -g npm@latest elm \
    && node --version && npm --version && elm --version
