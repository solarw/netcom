# Makefile for P2P Network with PyO3 Python Bindings

# Check if Poetry is installed, if not, install it
POETRY := $(shell command -v poetry 2> /dev/null)
VENV := $(shell poetry env info -p 2>/dev/null)

# Check if Maturin is available in the venv
MATURIN := $(shell poetry run command -v maturin 2> /dev/null)

.PHONY: all setup clean build develop test examples install-deps check-deps update-deps docs run run_server check_prompt_toolkit

# Default target - build the project
all: build

# Setup: install dependencies and prepare environment
setup: install-deps

# Clean build artifacts
clean:
	@echo "🧹 Cleaning up..."
	@cargo clean
	@rm -rf target/
	@rm -rf build/
	@rm -rf dist/
	@rm -rf *.egg-info/
	@rm -rf .pytest_cache/
	@find . -name "__pycache__" -type d -exec rm -rf {} +
	@find . -name "*.pyc" -delete
	@find . -name "*.pyo" -delete
	@find . -name "*.pyd" -delete
	@find . -name "*.so" -delete
	@find . -name "*.dll" -delete
	@find . -name "*.log" -delete
	@echo "✨ Clean complete"

# Install dependencies using Poetry
install-deps:
ifndef POETRY
	@echo "🔄 Poetry is not installed. Installing Poetry..."
	@curl -sSL https://install.python-poetry.org | python3 -
	@echo "✅ Poetry has been installed."
endif
	@echo "🔄 Installing Python dependencies with Poetry..."
	@poetry install
ifndef MATURIN
	@echo "🔄 Maturin not found, installing..."
	@poetry add --dev maturin
endif
	@echo "🔄 Installing prompt_toolkit for interactive console..."
	@poetry add prompt_toolkit
	@echo "✅ Dependencies installed"

# Build the project
build:
	@echo "🔨 Building the project with Maturin..."
	@PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 poetry run maturin build
	@echo "✅ Build complete"

# Development build (for local testing)
develop:
	@echo "🔨 Building development version with Maturin..."
	@PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 poetry run maturin develop
	@echo "Making p2p_network module available..."
	@mkdir -p p2p_network
	@if [ ! -f p2p_network/__init__.py ]; then \
		cp -f $(shell find . -name "__init__.py" -path "*/p2p_network/*" | head -n1) p2p_network/ 2>/dev/null || \
		echo 'from p2p_network_py import *' > p2p_network/__init__.py; \
	fi
	@echo "✅ Development build complete"

# Run tests
test:
	@echo "🧪 Running tests..."
	@poetry run pytest -xvs tests/
	@echo "✅ Tests complete"

# Example commands
examples: develop
	@echo "🚀 Running examples..."
	@echo "  - To run async node example: poetry run python examples/async_node.py"
	@echo "  - To run interactive node example: poetry run python examples/interactive_node.py"

run: develop
	@echo "🚀 Running interactive node application..."
	@PYTHONPATH=$(PWD) poetry run python examples/interactive_node.py

# Run the interactive node application in Kademlia server mode
run_server: develop
	@echo "🚀 Running interactive node application in Kademlia server mode..."
	@PYTHONPATH=$(PWD) poetry run python examples/interactive_node.py --kad-server --port 33333 --disable-mdns

start: 
	@echo "🚀 Running interactive node application..."
	@PYTHONPATH=$(PWD) poetry run python examples/interactive_node.py
	
start_server:
	@echo "🚀 Running interactive node application in Kademlia server mode..."
	@PYTHONPATH=$(PWD) poetry run python examples/interactive_node.py --kad-server --port 33333 --disable-mdns

# Check if prompt_toolkit is installed and install if not
check_prompt_toolkit:
	@echo "🔍 Checking for prompt_toolkit..."
	@if ! poetry run python -c "import prompt_toolkit" 2>/dev/null; then \
		echo "🔄 Installing prompt_toolkit..."; \
		poetry add prompt_toolkit; \
	else \
		echo "✅ prompt_toolkit is already installed"; \
	fi

# Check for outdated dependencies
check-deps:
	@echo "🔍 Checking for outdated dependencies..."
	@poetry show --outdated
	@cargo outdated --root-deps-only
	@echo "✅ Dependencies check complete"

# Update dependencies
update-deps:
	@echo "🔄 Updating dependencies..."
	@poetry update
	@cargo update
	@echo "✅ Dependencies updated"

# Generate documentation
docs:
	@echo "📚 Generating documentation..."
	@cargo doc --no-deps
	@echo "✅ Documentation generated"

# Release build
release: clean
	@echo "🚀 Building release package..."
	@poetry run maturin build --release
	@echo "✅ Release build complete. Check the 'target/wheels/' directory for the wheel file."

# Install the package
install: release
	@echo "📦 Installing the package..."
	@pip install --force-reinstall $(shell find target/wheels -name "*.whl" | sort -r | head -n1)
	@echo "✅ Package installed"