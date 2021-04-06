{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
    buildInput = with pkgs; [
        pkgconfig
        gcc
        stdenv.cc.cc.lib
        stdenv.cc.cc
        just
        rust-bindgen
        cargo
        cargo-release
    ];
}
