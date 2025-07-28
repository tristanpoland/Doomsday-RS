# Doomsday Certificate Monitor Makefile

.PHONY: help build test clean docker run dev lint fmt check install

# Default target
.DEFAULT_GOAL := help

# Variables
BINARY_SERVER := doomsday-server
BINARY_CLI := doomsday-cli
BUILD_DIR := target/release
DOCKER_IMAGE := doomsday-rs
VERSION := $(shell git describe --tags --always --dirty=+ 2>/dev/null || echo "development")

help: ## Show this help message
	@echo "Doomsday Certificate Monitor"
	@echo "============================"
	@echo ""
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build: ## Build the Rust binaries
	@echo "Building Rust binaries..."
	cargo build --release
	@echo "Built $(BINARY_SERVER) and $(BINARY_CLI) in $(BUILD_DIR)/"

build-frontend: ## Build the Next.js frontend
	@echo "Building frontend..."
	cd frontend && npm ci && npm run build

build-all: build build-frontend ## Build both backend and frontend

test: ## Run all tests
	@echo "Running Rust tests..."
	cargo test
	@echo "Running frontend tests..."
	cd frontend && npm test -- --passWithNoTests

test-rust: ## Run only Rust tests
	cargo test

test-frontend: ## Run only frontend tests
	cd frontend && npm test -- --passWithNoTests

clean: ## Clean build artifacts
	@echo "Cleaning build artifacts..."
	cargo clean
	cd frontend && rm -rf .next node_modules dist

dev: ## Run in development mode
	@echo "Starting development servers..."
	cargo watch -x run &
	cd frontend && npm run dev &
	wait

run: build ## Build and run the server
	@echo "Starting Doomsday server..."
	./$(BUILD_DIR)/$(BINARY_SERVER)

run-cli: build ## Build and run the CLI
	./$(BUILD_DIR)/$(BINARY_CLI) --help

docker: ## Build Docker image
	@echo "Building Docker image: $(DOCKER_IMAGE):$(VERSION)"
	docker build -t $(DOCKER_IMAGE):$(VERSION) .
	docker tag $(DOCKER_IMAGE):$(VERSION) $(DOCKER_IMAGE):latest

docker-compose-up: ## Start with docker-compose
	docker-compose up -d

docker-compose-down: ## Stop docker-compose
	docker-compose down

docker-compose-logs: ## View docker-compose logs
	docker-compose logs -f

lint: ## Run linting
	@echo "Running Rust linting..."
	cargo +stable clippy -- -D warnings
	@echo "Running frontend linting..."
	cd frontend && npm run lint

fmt: ## Format code
	@echo "Formatting Rust code..."
	cargo fmt
	@echo "Formatting frontend code..."
	cd frontend && npm run lint -- --fix

check: lint test ## Run linting and tests

install: build ## Install binaries to /usr/local/bin
	@echo "Installing binaries..."
	sudo cp $(BUILD_DIR)/$(BINARY_SERVER) /usr/local/bin/
	sudo cp $(BUILD_DIR)/$(BINARY_CLI) /usr/local/bin/doomsday
	@echo "Installed $(BINARY_SERVER) and doomsday to /usr/local/bin/"

install-frontend-deps: ## Install frontend dependencies
	cd frontend && npm ci

release: clean build-all docker ## Build release artifacts
	@echo "Built release version $(VERSION)"

# Development helpers
watch: ## Watch and rebuild on changes
	cargo watch -x build

watch-test: ## Watch and run tests on changes
	cargo watch -x test

logs: ## Show server logs (if running via docker-compose)
	docker-compose logs -f doomsday-backend

# Database/cache operations
refresh: ## Trigger cache refresh (requires running server)
	curl -X POST http://localhost:8111/v1/cache/refresh

info: ## Get server info (requires running server)
	curl http://localhost:8111/v1/info

# Configuration helpers
config-check: ## Validate configuration file
	@echo "Checking configuration..."
	./$(BUILD_DIR)/$(BINARY_SERVER) --help 2>/dev/null || echo "Binary not found, run 'make build' first"

config-example: ## Show example configuration
	@cat ddayconfig.yml

# Security
security-audit: ## Run security audit
	cargo +stable audit

update-deps: ## Update dependencies
	cargo update
	cd frontend && npm update

# Benchmarking (if needed)
bench: ## Run benchmarks
	cargo bench

# Documentation
docs: ## Generate and open documentation
	cargo doc --open

docs-frontend: ## Generate frontend documentation
	cd frontend && npm run build

# CI/CD helpers
ci-test: check ## Run CI tests
	@echo "All CI checks passed!"

version: ## Show version information
	@echo "Version: $(VERSION)"
	@echo "Git commit: $(shell git rev-parse HEAD 2>/dev/null || echo 'unknown')"