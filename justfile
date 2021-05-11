FEATURES := "--all-features"

debian-setup:
    echo "Installing dependencies..."
    sudo apt update && sudo apt-get install -y curl xz-utils build-essential gcc-multilib libclang-dev clang valgrind

build:
    rustc --version
    cargo --version

    cargo build --release --verbose {{FEATURES}}

test:
    rustc --version
    cargo --version

    # Limit test threads to 1 to show test name before execution.
    RUST_TEST_THREADS=1 cargo test --release --verbose {{FEATURES}}

lint:
    rustc --version
    cargo --version
    cargo clippy --version

    echo "Linting!"
    rustup component add rustfmt clippy

    echo "Checking formatting..."
    cargo fmt -- --check
    echo "Checking clippy..."
    cargo clippy

valgrind:
    echo "Checking for memory leaks..."
    find target/{{env_var_or_default("CARGO_BUILD_TARGET", "")}}/release/deps -maxdepth 1 -type f -executable | xargs valgrind --leak-check=full --error-exitcode=1
