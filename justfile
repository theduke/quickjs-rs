embed_dir := "./libquickjs-sys/embed/quickjs"

DOWNLOAD_URL := "https://bellard.org/quickjs/quickjs-2019-09-18.tar.xz"
FEATURES := ""

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

ci-debian-setup:
    echo "Installing dependencies..."
    apt update && apt-get install -y curl xz-utils build-essential gcc-multilib libclang-dev clang valgrind

ci-test:
    # Limit test threads to 1 to show test name before execution.
    RUST_TEST_THREADS=1 cargo test --verbose --features="{{FEATURES}}"

ci-lint:
    rustup component add rustfmt clippy
    echo "Linting!"

    echo "Checking formatting..."
    cargo fmt -- --check
    echo "Checking clippy..."
    cargo clippy

ci-valgrind:
    echo "Checking for memory leaks..."
    find target/debug -maxdepth 1 -type f -executable | xargs valgrind --leak-check=full --error-exitcode=1

ci-debian: ci-debian-setup ci-test ci-lint ci-valgrind

ci-macos-setup:
    echo "setup"

ci-macos: ci-macos-setup ci-test
