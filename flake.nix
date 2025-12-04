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

      pythonEnv = pkgs.python3.withPackages (ps:
        with ps; [
          matplotlib
          pandas
        ]);
    in {
      formatter = pkgs.alejandra;

      packages.default = pkgs.rustPlatform.buildRustPackage {
        pname = "twatch";
        version = "0.1.2";

        src = ./.;

        cargoLock.lockFile = ./Cargo.lock;

        nativeBuildInputs = [pkgs.makeWrapper];
        postInstall = ''
          cp src/graph.py $out/bin/graph.py
          wrapProgram $out/bin/twatch --prefix PATH : ${pkgs.lib.makeBinPath [pythonEnv]}
        '';
      };

      devShells.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          pythonEnv
          cargo
	  pkg-config
        ];
        buildInputs = with pkgs; [
          rustfmt
          rust-analyzer
          ruff
          rustc
	  gtk4
          gh
        ];

        shellHook = ''
        '';
      };
    });
}
