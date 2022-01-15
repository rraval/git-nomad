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
        doitlive
        figlet
        gdb
        rust-analyzer
        rustc
        rustfmt

        (pkgs.writeShellScriptBin "recordDemo" ''
          asciinema rec --overwrite -c 'doitlive play --commentecho --quiet --shell bash demo.doitlive.sh' demo.asciinema.cast
        '')
      ];
    };
  });
}
