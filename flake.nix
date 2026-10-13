{
  description = "hecate: a stacking Wayland compositor";

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
    crane.url = "git+https://github.com/ipetkov/crane?shallow=1";
    # Only used by the Home Manager module check.
    home-manager = {
      url = "git+https://github.com/nix-community/home-manager?shallow=1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      home-manager,
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
        hecate-shell = pkgs.callPackage ./nix/shell-package.nix { };
        default = hecate;
      });

      nixosModules.default = import ./nix/nixos-module.nix self;
      homeManagerModules.default = import ./nix/hm-module.nix self;

      overlays.default = final: _prev: {
        hecate = self.packages.${final.stdenv.hostPlatform.system}.hecate;
        hecate-shell = self.packages.${final.stdenv.hostPlatform.system}.hecate-shell;
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
          hecate-shell = self.packages.${pkgs.stdenv.hostPlatform.system}.hecate-shell;
          # docs/keybinds.md's shell table lists exactly the shell profile's binds (the base
          # table is checked by a Rust test).
          keybind-docs =
            pkgs.runCommand "keybind-docs"
              {
                nativeBuildInputs = [ pkgs.python3 ];
                binds = builtins.toJSON (builtins.attrNames (import ./nix/shell-binds.nix { shell = "/shell"; }));
                passAsFile = [ "binds" ];
              }
              ''
                python3 ${./nix/tests/keybind-docs.py} ${./docs/keybinds.md} "$bindsPath"
                touch $out
              '';
          # The Home Manager module with the shell: builds the home and validates the generated
          # config (shell binds included) with `hecate validate`.
          hm =
            (home-manager.lib.homeManagerConfiguration {
              inherit pkgs;
              modules = [
                self.homeManagerModules.default
                {
                  home = {
                    username = "alice";
                    homeDirectory = "/home/alice";
                    stateVersion = "25.11";
                  };
                  wayland.windowManager.hecate = {
                    enable = true;
                    shell.enable = true;
                    settings.binds."Mod+T".spawn = [ "foot" ];
                  };
                }
              ];
            }).activationPackage;
        }
        //
          removeAttrs
            (pkgs.callPackage ./nix/checks.nix {
              craneLib = crane.mkLib pkgs;
              common = common pkgs;
            })
            [
              "override"
              "overrideDerivation"
            ]
        // lib.optionalAttrs (pkgs.stdenv.hostPlatform.system == "x86_64-linux") {
          vm = import ./nix/tests/vm.nix { inherit self pkgs; };
          vm-shell = import ./nix/tests/vm-shell.nix { inherit self pkgs home-manager; };
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
