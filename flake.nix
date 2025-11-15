{
  description = "Tbench";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {nixpkgs, ...}: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs; [
      rustfmt
      rust-analyzer
      cargo
      gh
      ];

      shellHook = ''
      echo "Tbench project Devshell."
      rustfmt -V; \
      rust-analyzer --version; \
      cargo --version; \
      '';
    };
  };
}
