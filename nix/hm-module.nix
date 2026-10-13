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
  # Included in this order, later wins: the shell's wallpaper colours (matugen writes
  # ~/.config/hecate/colors.toml), what its settings page writes (shell.toml), then yours.
  includes =
    lib.optional (
      shellCfg.enable && shellCfg.theming.borders
    ) "${config.xdg.configHome}/hecate/colors.toml"
    ++ lib.optional shellCfg.enable "${config.xdg.configHome}/hecate/shell.toml"
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

      theming = {
        borders = lib.mkOption {
          type = lib.types.bool;
          default = !(cfg.settings ? border.color);
          defaultText = lib.literalExpression "!(settings ? border.color)";
          description = ''
            Colour hecate's borders from the wallpaper: the shell's matugen run writes
            `~/.config/hecate/colors.toml`, which the config includes. Off when
            `settings.border.color` is set, since the include would override it.
          '';
        };

        qt = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = ''
            Theme Qt apps from the wallpaper the way illogical-impulse does on Plasma: the KDE
            platform theme (`QT_QPA_PLATFORMTHEME=kde`) with the Darkly style, reading the
            colour scheme the shell writes into `~/.config/kdeglobals`. That file is seeded
            once (writable) if it doesn't exist.
          '';
        };

        folderIcons = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = ''
            Use Papirus with its folders recoloured to the wallpaper's primary colour as the
            GTK and KDE icon theme (`~/.local/share/icons/Papirus-Hecate*`).
          '';
        };
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

        # xdg-desktop-portal-wlr asks which output to share through a chooser. Its default looks
        # for slurp, wofi or bemenu on the PATH of a systemd user service, which has none of
        # them, so screen sharing failed. Pin a picker instead: the shell's (screens and windows,
        # with previews) when it runs, else fuzzel (screens only).
        xdg.configFile."xdg-desktop-portal-wlr/hecate".text =
          if shellCfg.enable then
            ''
              [screencast]
              chooser_type=simple
              chooser_cmd=${shellCfg.finalPackage}/bin/hecate-share-picker
            ''
          else
            ''
              [screencast]
              chooser_type=dmenu
              chooser_cmd=${lib.getExe pkgs.fuzzel} --dmenu --prompt "Share which screen? "
            '';

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
        ])
        ++ lib.optional shellCfg.theming.folderIcons pkgs.papirus-icon-theme
        ++ lib.optionals shellCfg.theming.qt [
          pkgs.darkly
          pkgs.kdePackages.qqc2-desktop-style
        ];
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
            Environment = lib.optional (!shellCfg.theming.folderIcons) "HECATE_FOLDER_COLORS=0";
            ExecStart = lib.getExe shellCfg.finalPackage;
            Restart = "on-failure";
            RestartSec = 1;
          };
          Install.WantedBy = [ "hecate-session.target" ];
        };
      })

      (lib.mkIf (shellCfg.enable && shellCfg.theming.qt) {
        # plasma-integration's platform theme reads kdeglobals and reloads on the shell's
        # D-Bus signal, as on Plasma.
        qt = {
          enable = true;
          platformTheme.name = lib.mkDefault "kde";
        };

        # The shell rewrites kdeglobals on every wallpaper change, so it can't be a store
        # symlink. Seed it from illogical-impulse's, minus fonts and icons nixpkgs lacks.
        home.activation.hecateKdeglobals = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
          kdeglobals="${config.xdg.configHome}/kdeglobals"
          if [ ! -e "$kdeglobals" ]; then
            run mkdir -p "${config.xdg.configHome}"
            run sed -e 's/Google Sans Flex/Rubik/g' \
              -e 's/^Theme=breeze-dark$/Theme=${
                if shellCfg.theming.folderIcons then "Papirus-Dark" else "breeze-dark"
              }/' \
              ${dots}/kdeglobals > "$kdeglobals"
          fi
        '';
      })
    ]
  );
}
