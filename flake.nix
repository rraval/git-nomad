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
        asciinema
        cargo
        cargo-outdated
        clippy
        figlet
        gdb
        rust-analyzer
        rustc
        rustfmt

        (pkgs.writeShellScriptBin "recNew" ''
          export HOST="$1"
          asciinema rec -i 0.3 -c 'bash --rcfile asciinema_env.sh' asciinema.cast
        '')

        (pkgs.writeShellScriptBin "recContinue" ''
          export HOST="$1"
          asciinema rec --append -i 0.3 -c 'bash --rcfile asciinema_env.sh' asciinema.cast
        '')
      ];
    };
  });
}
