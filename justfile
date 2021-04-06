embed_dir := "./libquickjs-sys/embed/quickjs"

DOWNLOAD_URL := "https://bellard.org/quickjs/quickjs-2021-03-27.tar.xz"
FEATURES := "--all-features"

download-new:
    test -d {{embed_dir}} && rm -r {{embed_dir}} || echo ""
    mkdir {{embed_dir}} && \
    curl -L {{DOWNLOAD_URL}} | tar xJv -C {{embed_dir}} --strip-components 1

download-cleanup:
    rm -r "{{embed_dir}}/doc" "{{embed_dir}}/examples" "{{embed_dir}}/tests"
    find "{{embed_dir}}" -type f | grep -E "\.(pdf|html|js|texi|sh)$" | xargs rm
    find "{{embed_dir}}" -type f | grep test | xargs rm

generate-bindings:
    (cd libquickjs-sys; bindgen wrapper.h -o embed/bindings.rs -- -I ./embed)
    # Update VERSION in README
    sed -i "s/**Embedded VERSION: .*/**Embedded VERSION: $(cat ./libquickjs-sys/embed/quickjs/VERSION)**/" ./libquickjs-sys/README.md

update-quickjs: download-new generate-bindings download-cleanup


debian-setup:
    echo "Installing dependencies..."
    sudo apt update && sudo apt-get install -y curl xz-utils build-essential gcc-multilib libclang-dev clang valgrind

build:
    rustc --version
    cargo --version

    cargo build --verbose {{FEATURES}}

test:
    rustc --version
    cargo --version

    # Limit test threads to 1 to show test name before execution.
    RUST_TEST_THREADS=1 cargo test --verbose {{FEATURES}}

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
    find target/debug/deps -maxdepth 1 -type f -executable | xargs valgrind --leak-check=full --error-exitcode=1
