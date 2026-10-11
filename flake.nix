{
  description = "hecate: a stacking Wayland compositor";

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
    crane.url = "git+https://github.com/ipetkov/crane?shallow=1";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      common =
        pkgs:
        pkgs.callPackage ./nix/common.nix {
          craneLib = crane.mkLib pkgs;
          rev = self.shortRev or self.dirtyShortRev or "unknown";
        };
    in
    {
      packages = forAllSystems (pkgs: rec {
        hecate = pkgs.callPackage ./nix/package.nix {
          craneLib = crane.mkLib pkgs;
          common = common pkgs;
        };
        default = hecate;
      });

      overlays.default = final: _prev: {
        hecate = self.packages.${final.stdenv.hostPlatform.system}.hecate;
      };

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.hecate ];
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            wayland-utils
            foot
            xwayland-satellite
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath (
            with pkgs;
            [
              libGL
              libxkbcommon
              wayland
            ]
          );
          # Software EGL for the headless render tests.
          __EGL_VENDOR_LIBRARY_FILENAMES = "${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
          LIBGL_ALWAYS_SOFTWARE = "1";
          GALLIUM_DRIVER = "llvmpipe";
        };
      });

      checks = forAllSystems (
        pkgs:
        {
          hecate = self.packages.${pkgs.stdenv.hostPlatform.system}.hecate;
        }
        // pkgs.callPackage ./nix/checks.nix {
          craneLib = crane.mkLib pkgs;
          common = common pkgs;
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
