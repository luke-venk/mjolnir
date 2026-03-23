#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Updating system..."
sudo apt update

echo "Installing core dependencies..."
sudo apt install -y \
  build-essential \
  curl \
  git \
  wget \
  cmake \
  pkg-config \
  libssl-dev \
  zlib1g-dev \
  libbz2-dev \
  libreadline-dev \
  libsqlite3-dev \
  llvm \
  libncurses5-dev \
  libncursesw5-dev \
  xz-utils \
  tk-dev \
  libffi-dev \
  liblzma-dev \
  python3-opencv \
  docker.io

echo "Starting Docker..."
sudo systemctl enable docker
sudo systemctl start docker

echo "Adding user to docker group..."
sudo usermod -aG docker $USER

echo "Installing pyenv..."
if [ ! -d "$HOME/.pyenv" ]; then
  curl https://pyenv.run | bash
fi

export PYENV_ROOT="$HOME/.pyenv"
export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init -)"

if ! grep -q 'pyenv init' ~/.bashrc 2>/dev/null; then
  cat <<'EOF' >> ~/.bashrc

# pyenv
export PYENV_ROOT="$HOME/.pyenv"
command -v pyenv >/dev/null || export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init -)"
EOF
fi

echo "Installing nvm..."
if [ ! -d "$HOME/.nvm" ]; then
  curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.4/install.sh | bash
fi

export NVM_DIR="$HOME/.nvm"
# shellcheck disable=SC1090
[ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"

echo "Installing rustup..."
if ! command -v rustup >/dev/null 2>&1; then
  curl https://sh.rustup.rs -sSf | sh -s -- -y
  export PATH="$HOME/.cargo/bin:$PATH"
fi

PYTHON_FILE="$SCRIPT_DIR/.python-version"
if [ -f "$PYTHON_FILE" ]; then
  PYTHON_VERSION="$(cat "$PYTHON_FILE" | tr -d '[:space:]')"
  echo "Installing Python $PYTHON_VERSION..."
  pyenv install -s "$PYTHON_VERSION"
  pyenv local "$PYTHON_VERSION"
else
  echo "No .python-version file found."
fi

NODE_FILE="$SCRIPT_DIR/.nvmrc"
if [ -f "$NODE_FILE" ]; then
  NODE_VERSION="$(cat "$NODE_FILE" | tr -d '[:space:]')"
  echo "Installing Node $NODE_VERSION..."
  nvm install "$NODE_VERSION"
  nvm use "$NODE_VERSION"
else
  echo "No .nvmrc file found."
fi

if [ -d "$SCRIPT_DIR/frontend" ]; then
  echo "Installing Node dependencies in 'frontend'..."
  cd "$SCRIPT_DIR/frontend"
  npm ci
else
  echo "Frontend directory not found, skipping npm install."
fi

echo "Installing Bazelisk..."
if ! command -v bazel >/dev/null 2>&1; then
    curl -L "https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-amd64" -o bazel
    chmod +x bazel
    sudo mv bazel /usr/local/bin/bazel
fi

echo "Generating Rust project..."
bazel run @rules_rust//tools/rust_analyzer:gen_rust_project

echo "Setup complete."
