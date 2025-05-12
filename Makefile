# Makefile for P2P Kademlia Discovery Test

# Check if Poetry is installed, if not, install it
POETRY := $(shell command -v poetry 2> /dev/null)

.PHONY: test build clean setup install logs

# Default target - run the Kademlia discovery test
test: setup
	@echo "Running Kademlia discovery tests..."
	@poetry run pytest -v -s tests/test_kad_discovery.py || (echo "Tests failed. Check the logs in *.log files"; exit 1)

# Build the Rust project
build:
	@echo "Building Rust project..."
	@cargo build || (echo "Build failed"; exit 1)

# Setup environment
setup: install build

# Install Python dependencies with Poetry
install:
ifndef POETRY
	@echo "Poetry is not installed. Installing poetry..."
	@curl -sSL https://install.python-poetry.org | python3 - || (echo "Failed to install Poetry"; exit 1)
	@echo "Poetry has been installed."
endif
	@echo "Installing Python dependencies..."
	@poetry install || (echo "Failed to install dependencies"; exit 1)

# Clean build artifacts
clean:
	@echo "Cleaning up..."
	@cargo clean
	@rm -rf .pytest_cache
	@find . -name "__pycache__" -type d -exec rm -rf {} +
	@find . -name "*.pyc" -delete
	@rm -f *.log

# Show logs
logs:
	@echo "Available logs:"
	@find . -name "*.log" -exec echo {} \;
	@echo "\nTo view a specific log file, use: cat <logfile>"

# Run test with detailed output and cleanup
test-full: clean test logs