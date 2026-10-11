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
  shellCfg = cfg.shell;
  toml = pkgs.formats.toml { };
  shellBinds = lib.optionalAttrs (shellCfg.enable && shellCfg.keybinds) (
    import ./shell-binds.nix { shell = shellCfg.finalPackage; }
  );
  # Shell layer rules first: later rules win, so yours override them.
  layerRules =
    lib.optionals (shellCfg.enable && shellCfg.layerRules) (import ./shell-layer-rules.nix)
    ++ (cfg.settings.layer_rules or [ ]);
  # The shell's settings page writes ~/.config/hecate/shell.toml; included last-but-yours.
  includes =
    lib.optional shellCfg.enable "${config.xdg.configHome}/hecate/shell.toml"
    ++ map toString cfg.extraConfigFiles;
  configFile = toml.generate "hecate-config.toml" (
    lib.recursiveUpdate (
      lib.optionalAttrs (shellBinds != { }) { shell_binds = shellBinds; }
      // lib.optionalAttrs (cfg.systemd.variables != [ ]) { session_variables = cfg.systemd.variables; }
    ) cfg.settings
    // lib.optionalAttrs (layerRules != [ ]) { layer_rules = layerRules; }
    // lib.optionalAttrs (includes != [ ]) { include = includes; }
  );
  dots = "${self}/shell/dots-hyprland/dots/.config";
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

    systemd.variables = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "XDG_VTNR" ];
      description = ''
        Extra environment variables imported into the systemd and D-Bus activation environment
        when hecate starts (WAYLAND_DISPLAY, DISPLAY, XDG_CURRENT_DESKTOP and the like always
        are). Same as the option other Home Manager window manager modules have.
      '';
    };

    shell = {
      enable = lib.mkEnableOption "the bundled desktop shell (end-4's illogical-impulse, end4-pC)";

      package = lib.mkOption {
        type = lib.types.package;
        default = self.packages.${pkgs.stdenv.hostPlatform.system}.hecate-shell;
        defaultText = lib.literalExpression "inputs.hecate.packages.\${system}.hecate-shell";
        description = "The shell package.";
      };

      extraPackages = lib.mkOption {
        type = lib.types.listOf lib.types.package;
        default = [ ];
        description = "More commands on the shell's PATH (hecate itself is always there).";
      };

      finalPackage = lib.mkOption {
        type = lib.types.package;
        readOnly = true;
        default = shellCfg.package.override {
          extraPackages = [ cfg.package ] ++ shellCfg.extraPackages;
        };
        defaultText = lib.literalExpression "shell.package with hecate and extraPackages on PATH";
        description = "The shell package actually installed.";
      };

      keybinds = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Add the shell's keybind profile (docs/keybinds.md) as `shell_binds`.";
      };

      layerRules = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = ''
          Add the shell's layer rules (blur behind panels, per-panel animations) before
          `settings.layer_rules`, which can override them.
        '';
      };

      dotfiles = lib.mkOption {
        type = lib.types.listOf (
          lib.types.enum [
            "matugen"
            "kde-material-you-colors"
            "Kvantum"
            "fontconfig"
            "fuzzel"
            "kitty"
            "foot"
            "fish"
          ]
        );
        default = [
          "matugen"
          "kde-material-you-colors"
          "Kvantum"
          "fuzzel"
        ];
        description = ''
          illogical-impulse dotfiles to install into `~/.config`. matugen's templates are what
          wallpaper theming needs; the terminal and shell configs are opt-in.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        xdg.configFile."hecate/config.toml".source = if cfg.validate then validated else configFile;

        systemd.user.targets.hecate-session = lib.mkIf cfg.systemd.enable {
          Unit = {
            Description = "hecate compositor session";
            BindsTo = [ "graphical-session.target" ];
            Wants = [ "graphical-session-pre.target" ];
            After = [ "graphical-session-pre.target" ];
          };
        };
      }

      (lib.mkIf shellCfg.enable {
        home.packages = [
          shellCfg.finalPackage
        ]
        ++ (with pkgs; [
          # Fonts and themes the shell's look assumes.
          rubik
          nerd-fonts.jetbrains-mono
          material-symbols
          twemoji-color-font
          adw-gtk3
          kdePackages.breeze-icons
          bibata-cursors
        ]);
        fonts.fontconfig.enable = true;

        # `qs -c end4-pC` (used by the shell itself) finds the config here.
        xdg.configFile = {
          "quickshell/end4-pC".source = "${shellCfg.finalPackage}/${shellCfg.finalPackage.config}";
        }
        // lib.genAttrs' shellCfg.dotfiles (
          name:
          lib.nameValuePair name {
            source = "${dots}/${name}";
            recursive = true;
          }
        );

        systemd.user.services.hecate-shell = {
          Unit = {
            Description = "hecate desktop shell (end4-pC)";
            PartOf = [ "graphical-session.target" ];
            After = [ "graphical-session.target" ];
            ConditionEnvironment = "WAYLAND_DISPLAY";
          };
          Service = {
            ExecStart = lib.getExe shellCfg.finalPackage;
            Restart = "on-failure";
            RestartSec = 1;
          };
          Install.WantedBy = [ "hecate-session.target" ];
        };
      })
    ]
  );
}
