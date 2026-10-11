# Home Manager module: wayland.windowManager.hecate
self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.wayland.windowManager.hecate;
  toml = pkgs.formats.toml { };
  configFile = toml.generate "hecate-config.toml" (
    cfg.settings
    // lib.optionalAttrs (cfg.extraConfigFiles != [ ]) { include = map toString cfg.extraConfigFiles; }
  );
  validated = pkgs.runCommand "hecate-config-validated.toml" { } ''
    ${lib.getExe cfg.package} validate ${configFile}
    cp ${configFile} $out
  '';
in
{
  options.wayland.windowManager.hecate = {
    enable = lib.mkEnableOption "the hecate Wayland compositor";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.hecate;
      defaultText = lib.literalExpression "inputs.hecate.packages.\${system}.hecate";
      description = "The hecate package (used for validation; install it with the NixOS module).";
    };

    settings = lib.mkOption {
      type = toml.type;
      default = { };
      example = lib.literalExpression ''
        {
          border = { width = 2; radius = 15; };
          workspaces = { scope = "per_output"; kind = "fixed"; count = 9; };
          binds."Mod+Return".spawn = [ "kitty" ];
        }
      '';
      description = ''
        hecate configuration as a Nix attrset, written to `~/.config/hecate/config.toml`.
        See docs/config.md for every option. The file is validated at build time.
      '';
    };

    extraConfigFiles = lib.mkOption {
      type = lib.types.listOf (lib.types.either lib.types.path lib.types.str);
      default = [ ];
      description = "Extra config files merged via `include` (later wins). Missing files are skipped.";
    };

    validate = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Run `hecate validate` on the generated config at build time.";
    };

    systemd.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Provide hecate-session.target, bound to graphical-session.target.";
    };
  };

  config = lib.mkIf cfg.enable {
    xdg.configFile."hecate/config.toml".source = if cfg.validate then validated else configFile;

    systemd.user.targets.hecate-session = lib.mkIf cfg.systemd.enable {
      Unit = {
        Description = "hecate compositor session";
        BindsTo = [ "graphical-session.target" ];
        Wants = [ "graphical-session-pre.target" ];
        After = [ "graphical-session-pre.target" ];
      };
    };
  };
}
