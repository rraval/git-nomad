{
  description = "git-nomad";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }: let
    eachSystem = fn: flake-utils.lib.eachDefaultSystem (system:
      fn nixpkgs.legacyPackages.${system}
    );
  in eachSystem (pkgs: {
    devShell = pkgs.mkShell {
      buildInputs = with pkgs; [
        cargo
        clippy
        rust-analyzer
        rustc
        rustfmt
      ];
    };
  });
}
