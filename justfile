
download:
    test -d ./quickjs-sys/quickjs && rm -r ./quickjs-sys/quickjs || echo ""
    mkdir ./quickjs-sys/quickjs && \
    curl -L https://bellard.org/quickjs/quickjs-2019-07-09.tar.xz | tar xJv -C ./quickjs-sys/quickjs --strip-components 1
