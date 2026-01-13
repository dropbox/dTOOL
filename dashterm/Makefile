# DashTerm Makefile
#
# Build and development commands for DashTerm

.PHONY: all setup build build-release test clean rust-build rust-test swift-build run

# Default target
all: build

# Initial setup
setup:
	@./scripts/setup.sh

# Debug build
build: rust-build swift-build

# Release build
build-release:
	@./scripts/build_rust.sh release
	@xcodebuild -project DashTerm.xcodeproj -scheme DashTerm -configuration Release build

# Build Rust library only
rust-build:
	@./scripts/build_rust.sh debug

# Build Swift app only (assumes Rust library exists)
swift-build:
	@xcodebuild -project DashTerm.xcodeproj -scheme DashTerm -configuration Debug build

# Run all tests
test: rust-test swift-test

# Rust tests only
rust-test:
	@cd dashterm-core && cargo test --all

# Swift tests only
swift-test:
	@tmp=$$(mktemp); \
	if xcodebuild test -project DashTerm.xcodeproj -scheme DashTerm -destination 'platform=macOS' -quiet >$$tmp 2>&1; then \
		rm -f $$tmp; \
	else \
		if grep -q "Scheme DashTerm is not currently configured for the test action" $$tmp; then \
			echo "Swift tests skipped: scheme DashTerm has no test action configured."; \
			rm -f $$tmp; \
		else \
			cat $$tmp; \
			rm -f $$tmp; \
			exit 1; \
		fi; \
	fi

# Lint and format
lint:
	@cd dashterm-core && cargo clippy --all -- -D warnings
	@cd dashterm-core && cargo fmt --all -- --check

# Format code
fmt:
	@cd dashterm-core && cargo fmt --all

# Clean build artifacts
clean:
	@cd dashterm-core && cargo clean
	@xcodebuild -project DashTerm.xcodeproj -scheme DashTerm clean 2>/dev/null || true
	@rm -f DashTerm/Bridge/libdashterm_ffi.a

# Run the app
run: build
	@open -a "$(shell find ~/Library/Developer/Xcode/DerivedData -name 'DashTerm.app' -type d 2>/dev/null | head -1)"

# Generate documentation
docs:
	@cd dashterm-core && cargo doc --all --no-deps --open

# Build universal binary for distribution
universal:
	@./scripts/build_rust.sh release --universal

# Watch for changes and rebuild (requires cargo-watch)
watch:
	@cd dashterm-core && cargo watch -x 'build -p dashterm-ffi'
