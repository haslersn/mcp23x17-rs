import ./default.nix {
    crossSystem = (import <nixpkgs> {}).lib.systems.examples.aarch64-multiplatform;
}
