{
  description = "Twatch - btop-grade temperature monitoring and graphing TUI";

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
      in
      {
        formatter = pkgs.alejandra;

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "twatch";
          version = "0.2.1";

          src = ./.;

          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = with pkgs; [
            pkg-config
            installShellFiles
          ];
          buildInputs = with pkgs; [ ];
          postInstall = ''
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
            fish
          ];

          shellHook = ''
            rust-analyzer --version; gh --version; cargo --version; echo "Loaded!"
          '';
        };
      }
    );
}
