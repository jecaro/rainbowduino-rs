{
  description = "rainbowduino-rs: Firmware for the Rainbowduino v2.0";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
  };

  outputs = { self, nixpkgs, fenix }:
    let
      systems = [ "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems f;

      perSystem = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          rustToolchain = fenix.packages.${system}.fromToolchainFile {
            file = ./slave/rust-toolchain.toml;
            sha256 = "DnyK5MS+xYySA+csnnMogu2gtEfyiy10W0ATmAvmjGg=";
          };

          # Toolchain needed to build+flash the firmware, used by both the
          # devShell and the flash script.
          toolchain = [
            rustToolchain
            pkgs.stdenv.cc # already included in the shell, here for simplicity
            pkgs.pkgsCross.avr.buildPackages.gcc
            pkgs.avrdude
          ];
        in
        { inherit pkgs toolchain; });
    in
    {
      devShells = forAllSystems (system:
        let
          inherit (perSystem.${system}) pkgs toolchain;
        in
        {
          default = pkgs.mkShell {
            buildInputs = toolchain ++ [
              fenix.packages.${system}.latest.rustfmt
            ];
          };
        });

      packages = forAllSystems (system:
        let
          inherit (perSystem.${system}) pkgs toolchain;
        in
        {
          flash-slave = pkgs.writeShellApplication {
            name = "flash-slave";
            runtimeInputs = toolchain;
            text = ''exec ./scripts/flash-slave.sh "$@"'';
          };

          flash-master = pkgs.writeShellApplication {
            name = "flash-master";
            runtimeInputs = toolchain;
            text = ''exec ./scripts/flash-master.sh "$@"'';
          };

          ci = pkgs.writeShellApplication {
            name = "ci";
            runtimeInputs = toolchain;
            text = ''exec ./scripts/ci.sh'';
          };
        });
    };
}
