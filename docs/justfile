# Docs justfile
TOP := `git rev-parse --show-toplevel`

# Build the documentation site
build:
    npm run build

# Start development server
dev:
    npm run start

# Serve built site locally
serve:
    npm run serve

# Clear cache and build artifacts
clean:
    npm run clear

# Type check
typecheck:
    npm run typecheck

# Install dependencies
install:
    npm install

# Build and serve (for testing production build)
test-build: build serve
