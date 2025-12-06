{
  description = "Twatch package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      formatter = pkgs.alejandra;

      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "twatch";
        version = "0.1.5";

        src = ./.;

        cargoLock.lockFile = ./Cargo.lock;
        nativeBuildInputs = with pkgs; [
          pkg-config
        ];
        buildInputs = with pkgs; [
          gtk4
        ];
      };

      devShells.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          cargo
          pkg-config
        ];
        buildInputs = with pkgs; [
          rustfmt
          rust-analyzer
          rustc
          gtk4
          gh
        ];

        shellHook = ''
          rustc --version; rust-analyzer --version; gtk4 --version; gh --version; cargo --version; echo "Loaded!"; fish
        '';
      };
    });
}
