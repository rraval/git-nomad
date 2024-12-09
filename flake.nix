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
  in eachSystem (pkgs: let
    gitNomadPkg = with pkgs; rustPlatform.buildRustPackage rec {
      pname = "git-nomad";
      version = self.shortRev or self.dirtyShortRev;

      src = self;

      cargoLock = {
        lockFile = ./Cargo.lock;
      };

      buildInputs = lib.optionals stdenv.isDarwin [
        darwin.apple_sdk.frameworks.SystemConfiguration
      ];

      preBuild = ''
        export GIT_NOMAD_BUILD_VERSION='${version}'
      '';

      nativeCheckInputs = [
        git
      ];
    };
  in {
    devShells.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        asciinema
        cargo
        cargo-llvm-cov
        cargo-outdated
        clippy
        gdb
        just
        rustc
        rustfmt
        shellcheck
      ];

      shellHook = ''
        export LLVM_COV='${pkgs.cargo-llvm-cov.LLVM_COV}'
        export LLVM_PROFDATA='${pkgs.cargo-llvm-cov.LLVM_PROFDATA}'
      '';
    };

    checks.git-nomad = gitNomadPkg;
    packages.default = gitNomadPkg;
    apps.default = {
      type = "app";
      program = "${gitNomadPkg}/bin/git-nomad";
    };
  });
}
