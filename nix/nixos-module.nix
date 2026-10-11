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
  };

  config = lib.mkIf cfg.enable {
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
  };
}
