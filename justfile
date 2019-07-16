embed_dir := "./quickjs-sys/embed/quickjs"

download:
    test -d {{embed_dir}} && rm -r {{embed_dir}} || echo ""
    mkdir {{embed_dir}} && \
    curl -L https://bellard.org/quickjs/quickjs-2019-07-09.tar.xz | tar xJv -C {{embed_dir}} --strip-components 1
    find {{embed_dir}} | grep ""

    find {{embed_dir}} | grep -E "pdf|html|texi" | xargs rm
    rm -r {{embed_dir}}/doc

install: download
    cd {{embed_dir}} && sudo make install

ci-debian-setup:
    echo "Installing dependencies..."
    apt update && apt-get install -y curl xz-utils build-essential gcc-multilib libclang-dev clang

ci-test:
    cargo test --verbose

ci-debian: ci-debian-setup ci-test

ci-macos-setup:
    echo "setup"

ci-macos: ci-macos-setup ci-test
