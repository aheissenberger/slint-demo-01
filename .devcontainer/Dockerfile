FROM mcr.microsoft.com/devcontainers/base:trixie

ENV DEBIAN_FRONTEND=noninteractive \
    DISPLAY=:1 \
    SLINT_BACKEND=winit-software \
    XDG_RUNTIME_DIR=/tmp/runtime-dir \
    PULSE_SERVER=unix:/tmp/pulse/native \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo

# libinput10 is required by the VS Code Slint extension's bundled Live Preview
# LSP binary (slint-lsp) on aarch64; without it "Show Preview" fails with
# "command 'slint/showPreview' not found".
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash ca-certificates curl git gnupg lsb-release pkg-config build-essential \
    ripgrep fd-find jq tree shellcheck python3 unzip \
    libx11-dev libxext-dev libxft-dev libxrandr-dev libxrender-dev libxcb1-dev \
    libx11-xcb-dev libxcursor-dev xinput libssl-dev libglib2.0-dev libgtk-3-dev libasound2-dev \
    libxkbcommon-x11-0 libinput10 \
    xvfb xauth x11vnc openbox net-tools websockify novnc supervisor imagemagick x11-utils dbus-x11 \
    lldb \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /tmp && curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain 1.98.1 \
    && /root/.cargo/bin/rustup component add --toolchain 1.98.1 rustfmt clippy rust-analyzer \
    && /root/.cargo/bin/cargo install cargo-watch --locked

ARG ACTIONLINT_VERSION=1.7.7
RUN set -eux; \
    case "$(uname -m)" in \
      aarch64) actionlint_arch=arm64 ;; \
      x86_64) actionlint_arch=amd64 ;; \
      *) echo "Unsupported actionlint architecture: $(uname -m)" >&2; exit 1 ;; \
    esac; \
    curl -fsSL "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/actionlint_${ACTIONLINT_VERSION}_linux_${actionlint_arch}.tar.gz" \
      | tar -xz -C /usr/local/bin actionlint

RUN ln -sf /usr/bin/fdfind /usr/local/bin/fd

RUN ln -sf /root/.cargo/bin/cargo /usr/local/bin/cargo \
    && ln -sf /root/.cargo/bin/rustc /usr/local/bin/rustc \
    && ln -sf /root/.cargo/bin/rustfmt /usr/local/bin/rustfmt \
    && ln -sf /root/.cargo/bin/cargo-clippy /usr/local/bin/cargo-clippy \
    && ln -sf /root/.cargo/bin/rust-analyzer /usr/local/bin/rust-analyzer

WORKDIR /workspace
COPY . /workspace

RUN test -f /usr/share/novnc/vnc.html && \
    mkdir -p /var/run/dbus && touch /var/run/dbus/pid

CMD ["/bin/bash"]
