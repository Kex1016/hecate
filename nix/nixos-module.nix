# NixOS module: programs.hecate
self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.hecate;
in
{
  options.programs.hecate = {
    enable = lib.mkEnableOption "the hecate Wayland compositor";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.hecate;
      defaultText = lib.literalExpression "inputs.hecate.packages.\${system}.hecate";
      description = "The hecate package.";
    };

    xwayland.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Install xwayland-satellite so X11 apps (Steam, most games) work.";
    };

    gaming.enable = lib.mkEnableOption "32-bit graphics drivers and gamescope";

    fonts.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Install the haiiro fonts system-wide (IBM VGA 8x16, Jacquard 24, GNU Unifont as the
        glyph fallback) with their fontconfig fallback order, so the login screen and every
        program can use the shell's faces.
      '';
    };

    shell.enable = lib.mkEnableOption ''
      the system services the bundled shell relies on (lock screen PAM, geoclue, upower,
      NetworkManager, i2c for ddcutil, ydotool, bluetooth). Turn the shell itself on with the
      Home Manager option `wayland.windowManager.hecate.shell.enable`
    '';
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        environment.systemPackages = [
          cfg.package
        ]
        ++ lib.optional cfg.xwayland.enable pkgs.xwayland-satellite;

        services.displayManager.sessionPackages = [ cfg.package ];
        systemd.packages = [ cfg.package ];

        hardware.graphics.enable = lib.mkDefault true;
        hardware.graphics.enable32Bit = lib.mkIf cfg.gaming.enable (lib.mkDefault true);
        programs.gamescope.enable = lib.mkIf cfg.gaming.enable (lib.mkDefault true);

        security.polkit.enable = lib.mkDefault true;
        programs.dconf.enable = lib.mkDefault true;
        services.graphical-desktop.enable = lib.mkDefault true;

        xdg.portal = {
          enable = lib.mkDefault true;
          extraPortals = [
            pkgs.xdg-desktop-portal-wlr
            pkgs.xdg-desktop-portal-gtk
          ];
          configPackages = [ cfg.package ];
        };
      }

      (lib.mkIf cfg.fonts.enable {
        fonts.packages = [ self.packages.${pkgs.stdenv.hostPlatform.system}.haiiro-fonts ];
        fonts.fontconfig.confPackages = [ self.packages.${pkgs.stdenv.hostPlatform.system}.haiiro-fonts ];
      })

      (lib.mkIf cfg.shell.enable {
        # QuickShell's lock screen authenticates through PAM.
        security.pam.services.quickshell = { };
        services.geoclue2.enable = lib.mkDefault true;
        services.upower.enable = lib.mkDefault true;
        networking.networkmanager.enable = lib.mkDefault true;
        hardware.i2c.enable = lib.mkDefault true;
        hardware.bluetooth.enable = lib.mkDefault true;
        programs.ydotool.enable = lib.mkDefault true;
        services.gnome.gnome-keyring.enable = lib.mkDefault true;
      })
    ]
  );
}
