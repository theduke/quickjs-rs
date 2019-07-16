
download:
    test -d ./quickjs-sys/quickjs && rm -r ./quickjs-sys/quickjs || echo ""
    mkdir ./quickjs-sys/quickjs && \
    curl -L https://bellard.org/quickjs/quickjs-2019-07-09.tar.xz | tar xJv -C ./quickjs-sys/quickjs --strip-components 1

install: download
    cd quickjs-sys/quickjs && sudo make install

ci-setup-debian:
    echo "Installing dependencies..."
    sudo apt update && sudo apt-get install -y curl xz-utils build-essential gcc-multilib libclang-dev clang

ci-test:
    cargo test --verbose

ci-debian: ci-setup-debian install ci-test

ci-macos-setup:
    echo "setup"

ci-macos: ci-macos-setup install ci-test
