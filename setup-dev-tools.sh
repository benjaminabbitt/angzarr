#!/usr/bin/env bash
set -euo pipefail

# Install asdf and common language runtimes on Debian

echo "Installing dependencies..."
sudo apt-get update
sudo apt-get install -y curl git build-essential libssl-dev zlib1g-dev \
  libbz2-dev libreadline-dev libsqlite3-dev libncursesw5-dev xz-utils \
  tk-dev libxml2-dev libxmlsec1-dev libffi-dev liblzma-dev unzip

# Install asdf
echo "Installing asdf..."
git clone https://github.com/asdf-vm/asdf.git ~/.asdf --branch v0.14.1

export ASDF_DIR="$HOME/.asdf"
# shellcheck source=/dev/null
. "$ASDF_DIR/asdf.sh"

# Add plugins
echo "Adding asdf plugins..."
asdf plugin add nodejs
asdf plugin add python
asdf plugin add golang
asdf plugin add rust
asdf plugin add terraform

# Install latest versions
echo "Installing runtimes (this may take a while)..."
asdf install nodejs latest
asdf install python latest
asdf install golang latest
asdf install rust latest
asdf install terraform latest

# Set global defaults
asdf global nodejs latest
asdf global python latest
asdf global golang latest
asdf global rust latest
asdf global terraform latest

# Add to shell
echo '. "$HOME/.asdf/asdf.sh"' >> ~/.bashrc
echo '. "$HOME/.asdf/completions/asdf.bash"' >> ~/.bashrc

# Verify
echo ""
echo "Installed versions:"
asdf current

echo ""
echo "Run 'source ~/.bashrc' or start a new shell to use asdf."
