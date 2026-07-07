{
  description = "Twatch package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        pythonWithMatplotlib = pkgs.python3.withPackages (ps: [ ps.matplotlib ]);
      in
      {
        formatter = pkgs.alejandra;

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "twatch";
          version = "1.0.0";

          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [
            pkg-config
            makeBinaryWrapper
            installShellFiles
          ];
          buildInputs = with pkgs; [
            pythonWithMatplotlib
          ];
          postInstall = ''
            cp ${./plot.py} $out/bin/plot.py
            wrapProgram $out/bin/twatch --prefix PATH : ${pythonWithMatplotlib}/bin
            installShellCompletion --cmd twatch --bash <($out/bin/twatch completions bash)
            installShellCompletion --cmd twatch --zsh  <($out/bin/twatch completions zsh)
            installShellCompletion --cmd twatch --fish <($out/bin/twatch completions fish)
          '';
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
            gh
            pythonWithMatplotlib
          ];

          shellHook = ''
            rust-analyzer --version; gh --version; cargo --version; echo "Loaded!"; fish
          '';
        };
      }
    );
}
