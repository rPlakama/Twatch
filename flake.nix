{
  description = "Tbench";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {nixpkgs, ...}: let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
  in {
    formatter.x86_64-linux = nixpkgs.legacyPackages.x86_64-linux.alejandra;
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        # Sensors
        rustfmt
        rust-analyzer
        cargo
        gh
        # Datahandling
        python313
        python313Packages.pandas
        python313Packages.matplotlib
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
