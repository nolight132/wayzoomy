{
  description = "Minimal Wayland zoom utility";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];

      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        wayzoomy = pkgs.rustPlatform.buildRustPackage {
          pname = "wayzoomy";
          version = "0.1.0";

          src = self;

          cargoHash = "sha256-8RcekMhOuhP4eY0gsuZi3UtmukF2VMcBj5skHbYU/PU=";

          meta = {
            description = "Minimal Wayland zoom utility";
            homepage = "https://github.com/nolight132/wayzoomy";
            license = pkgs.lib.licenses.unlicense;
            mainProgram = "wayzoomy";
            platforms = pkgs.lib.platforms.linux;
          };
        };

        default = wayzoomy;
      });

      apps = forAllSystems (pkgs: rec {
        wayzoomy = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.wayzoomy}/bin/wayzoomy";
        };

        default = wayzoomy;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.rustc
            pkgs.rustfmt
            pkgs.clippy
          ];
        };
      });
    };
}
