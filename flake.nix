{
  description = "Library for working with Northstar mods";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    fenix-flake = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    fenix-flake,
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {inherit system;};
    in {
      formatter = pkgs.alejandra;

      devShells.default = pkgs.mkShell {
        nativeBuildInputs = [pkgs.rustc];
        packages = with pkgs; [cargo git-cliff rust-analyzer clippy];
      };

      devShells.coverage = let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [fenix-flake.overlays.default];
        };
      in
        pkgs.mkShell {
          # nativeBuildInputs = [pkgs.rustc];
          packages = with pkgs; [
            grcov
            (fenix.complete.withComponents [
              "cargo"
              "rustc"
              "llvm-tools-preview"
            ])
          ];
        };
    });
}
