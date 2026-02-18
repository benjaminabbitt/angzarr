#!/usr/bin/env bash
# Install Java 21 (Temurin/Adoptium) for Debian/Ubuntu
set -euo pipefail

echo "Installing Java 21 (Eclipse Temurin)..."

# Install prerequisites
sudo apt-get update
sudo apt-get install -y wget apt-transport-https gpg

# Add Adoptium repository
wget -qO - https://packages.adoptium.net/artifactory/api/gpg/key/public | sudo gpg --dearmor -o /usr/share/keyrings/adoptium.gpg
echo "deb [signed-by=/usr/share/keyrings/adoptium.gpg] https://packages.adoptium.net/artifactory/deb $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/adoptium.list

# Install Java 21
sudo apt-get update
sudo apt-get install -y temurin-21-jdk

# Verify installation
java -version
javac -version

echo "Java 21 installed successfully!"
echo "JAVA_HOME should be: /usr/lib/jvm/temurin-21-jdk-amd64"
