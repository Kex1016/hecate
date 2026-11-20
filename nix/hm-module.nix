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
  haiiro = shellCfg.enable && shellCfg.theming.compositor;
  # The haiiro look for what the compositor draws (Rosé Pine): no blur, square windows in
  # a 5px double frame (2px line, 1px gap, 2px line) with their name in a notch, and
  # nothing that slides or fades. Things pixelate in and out in four steps, workspace
  # switches, programmatic moves and border colour changes are cuts, and the login splash
  # hands over with one. `settings` overrides any of it.
  pixelate = {
    style = "pixelate";
    ms = 500;
    curve = "stepped";
    cell = 16;
    fade = false;
  };
  haiiroSettings = {
    general.background_color = "#191724";
    blur.enable = false;
    border = {
      width = 5;
      style = "double";
      radius = 0;
      background = "#1f1d2e";
      color = {
        active = "#31748f";
        inactive = "#403d52";
        urgent = "#eb6f92";
      };
      label = {
        enable = true;
        color = "#e0def4";
      };
      tint = {
        gold = "#f6c177";
        love = "#eb6f92";
        foam = "#9ccfd8";
      };
    };
    animations = {
      window_open = pixelate;
      window_close = pixelate;
      layer_open = pixelate;
      layer_close = pixelate;
      window_move.style = "none";
      border.style = "none";
      workspace_switch.style = "none";
    };
    splash.handover = "cut";
    # The live blob background is the desktop until you pick wallpaper mode (the shell
    # switches it over IPC). Set here too so the first frames, before the shell is up, are
    # blobs and not the bare ground colour; `resume` carries on from the login screen's.
    background.blobs = {
      enable = true;
    }
    // lib.optionalAttrs (shellCfg.blobs.resume != null) { inherit (shellCfg.blobs) resume; };
  };
  # With that look a panel appears the way everything does: the shell's per-panel slides,
  # fades and pop-ins are left out (a rule that says "none" still means it).
  shellLayerRule =
    rule:
    if haiiro && (rule.animation or "none") != "none" then removeAttrs rule [ "animation" ] else rule;
  # Shell layer rules first: later rules win, so yours override them.
  layerRules =
    lib.optionals (shellCfg.enable && shellCfg.layerRules) (
      # A rule that was only an animation has nothing left to say.
      lib.filter (rule: removeAttrs rule [ "match" ] != { }) (
        map shellLayerRule (import ./shell-layer-rules.nix)
      )
    )
    ++ (cfg.settings.layer_rules or [ ]);
  # Included in this order, later wins: what the shell's settings page writes (shell.toml),
  # then yours. The colours are haiiro's and fixed, so nothing is generated from the wallpaper.
  includes =
    lib.optional shellCfg.enable "${config.xdg.configHome}/hecate/shell.toml"
    ++ map toString cfg.extraConfigFiles;
  configFile = toml.generate "hecate-config.toml" (
    lib.recursiveUpdate (
      lib.optionalAttrs haiiro haiiroSettings
      // lib.optionalAttrs (shellBinds != { }) { shell_binds = shellBinds; }
      // lib.optionalAttrs (cfg.systemd.variables != [ ]) { session_variables = cfg.systemd.variables; }
      # The shell tells hecate when it's up, which ends the login splash.
      // lib.optionalAttrs shellCfg.enable {
        splash = lib.optionalAttrs haiiro haiiroSettings.splash // {
          enable = true;
        };
      }
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
  imports = [
    (lib.mkRemovedOptionModule [ "wayland" "windowManager" "hecate" "shell" "theming" "borders" ] ''
      The shell no longer colours borders from the wallpaper (matugen is gone): haiiro's frame
      colours are fixed, and `settings.border.color` overrides them.
    '')
  ];

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
          `settings.layer_rules`, which can override them. With `theming.compositor` the
          per-panel animations are left out: panels pixelate like everything else.
        '';
      };

      theming = {
        compositor = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = ''
            Give what the compositor draws the haiiro look: blur off, square windows in a
            5px double pine frame with "app: title" in a notch on its top edge, windows and
            layers that pixelate in and out in four steps, and cuts for workspace switches,
            border colour changes and the login splash. These are defaults under
            `settings`, which overrides any of them. The shell's per-panel slide, fade and
            pop-in layer rules are left out with it.
          '';
        };

        qt = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = ''
            Theme Qt apps the way Plasma does: the KDE platform theme
            (`QT_QPA_PLATFORMTHEME=kde`) with the Darkly style, reading the colour scheme in
            `~/.config/kdeglobals`. That file is seeded once (writable) if it doesn't exist; the
            shell's settings change its icons and fonts, never its colours.
          '';
        };

        folderIcons = lib.mkOption {
          type = lib.types.bool;
          default = true;
          description = ''
            Use Papirus-Dark as the KDE icon theme (seeded into `kdeglobals`) and install
            Papirus. The shell no longer recolours its folders from the wallpaper.
          '';
        };
      };

      blobs.resume = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = "/run/hecate-greetd/blobs.json";
        description = ''
          File the blob background carries on from at startup, so the desktop's first blob
          frame follows the login screen's last one. hecate-greetd writes it when you log in;
          a missing file just starts the blobs afresh. `null` leaves it out.
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
        default = [ "fuzzel" ];
        description = ''
          illogical-impulse dotfiles to install into `~/.config`. The shell no longer themes
          anything from the wallpaper, so matugen's, kde-material-you-colors' and Kvantum's are
          opt-in like the terminal and shell configs.
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
        ])
        # The haiiro faces for every other program, with their fallback order (the shell
        # loads its own copies).
        ++ [ self.packages.${pkgs.stdenv.hostPlatform.system}.haiiro-fonts ]
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

        # The shell's settings write icons and fonts into kdeglobals, so it can't be a store
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
