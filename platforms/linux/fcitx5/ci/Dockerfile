# Headless build environment only. This image does not run or install an IME.
FROM ubuntu:24.04@sha256:24737cef17f4e22b1977df7d9a05e77c4e28b7da5b8911f8d8ccda136bf3573c
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install --no-install-recommends -y \
    build-essential ca-certificates cmake curl ninja-build pkg-config \
    libfcitx5core-dev libfcitx5config-dev libfcitx5utils-dev \
    libxkbcommon-dev nlohmann-json3-dev \
    && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain 1.92.0
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /src
CMD ["bash", "/src/platforms/linux/fcitx5/ci/check.sh"]
