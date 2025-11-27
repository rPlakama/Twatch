{
  description = "Twatch package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        pythonEnv = pkgs.python3.withPackages (ps: with ps; [
          pandas
          matplotlib
        ]);
      in
      {
        formatter = pkgs.alejandra;

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "twatch";
          version = "0.1.0";

          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ pkgs.makeWrapper ];
          postInstall = ''
            cp src/graph.py $out/bin/graph.py
            wrapProgram $out/bin/twatch --prefix PATH : ${pkgs.lib.makeBinPath [ pythonEnv ]}
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustfmt
            rust-analyzer
            cargo
            rustc
            gh
            pythonEnv
          ];

          shellHook = ''
	  fish
          '';
        };
    });
}
