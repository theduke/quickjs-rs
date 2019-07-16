
download:
    test -d quickjs && rm -r quickjs || echo ""
    mkdir quickjs && \
    curl -L https://bellard.org/quickjs/quickjs-2019-07-09.tar.xz | tar xJv -C quickjs --strip-components 1
