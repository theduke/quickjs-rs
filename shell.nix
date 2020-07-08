{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
    buildInputs = with pkgs; [
        gcc
        just
        rust-bindgen
        cargo
        cargo-release
    ];
}
