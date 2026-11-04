# The bundled desktop shell: end4-pC (QuickShell) patched for hecate, with its Qt modules,
# runtime tools and Python environment. See docs/shell.md.
{
  lib,
  stdenv,
  runCommand,
  makeWrapper,
  quickshell,
  qt6,
  kdePackages,
  python3,
  gsettings-desktop-schemas,
  pkgs,
  # More commands for the shell's PATH (the Home Manager module adds hecate itself).
  extraPackages ? [ ],
}:
let
  rev = "26fe30f";

  pythonEnv = python3.withPackages (
    ps: with ps; [
      pillow
      pywayland
      psutil
      materialyoucolor
      # Moved out of python3Packages to pkgs/by-name (still a Python package).
      (pkgs.kde-material-you-colors or ps.kde-material-you-colors)
      libsass
      material-color-utilities
      setproctitle
      click
      loguru
      pycairo
      pygobject3
      tqdm
      numpy
      opencv4
      google-auth
      requests
    ]
  );

  # The shell's scripts `source $ILLOGICAL_IMPULSE_VIRTUAL_ENV/bin/activate`; a Nix Python env
  # has no activate script, so provide one.
  venv = runCommand "hecate-shell-python" { } ''
    mkdir -p $out/bin
    ln -s ${pythonEnv}/bin/* $out/bin/
    cat > $out/bin/activate <<EOF
    export VIRTUAL_ENV=$out
    export PATH=${pythonEnv}/bin:\$PATH
    deactivate() { :; }
    EOF
  '';

  # Commands the QML and scripts run (see shell/dots-hyprland/sdata/deps-info.md), minus the
  # Hyprland-only ones (hyprshot, hyprsunset, hypridle); night light uses wlsunset.
  runtimeTools =
    with pkgs;
    [
      bash
      coreutils
      findutils
      gnugrep
      gnused
      gawk
      procps
      psmisc
      util-linux
      bc
      jq
      curl
      wget
      ripgrep
      file
      libnotify
      glib
      xdg-user-dirs
      dbus
      # The QML calls `python3` directly for some scripts (colour schemes, ...).
      pythonEnv
      # audio
      cava
      lxqt.pavucontrol-qt
      wireplumber
      playerctl
      # backlight
      brightnessctl
      ddcutil
      # theming
      matugen
      imagemagick
      kdePackages.kconfig
      # capture
      grim
      slurp
      swappy
      wf-recorder
      tesseract
      hyprpicker
      wl-clipboard
      cliphist
      ffmpeg
      # widgets and tools
      fuzzel
      translate-shell
      libqalculate
      songrec
      upower
      wtype
      ydotool
      networkmanager
      bluez
      wlsunset
      mpvpaper
      quickshell
    ]
    ++ extraPackages;

  qtModules = [
    qt6.qtbase
    qt6.qtdeclarative
    qt6.qt5compat
    qt6.qtimageformats
    qt6.qtmultimedia
    qt6.qtpositioning
    qt6.qtquicktimeline
    qt6.qtsensors
    qt6.qtsvg
    qt6.qttranslations
    qt6.qtvirtualkeyboard
    qt6.qtwayland
    kdePackages.qtlocation
    kdePackages.kirigami
    kdePackages.syntax-highlighting
    kdePackages.kdialog
  ];
in
stdenv.mkDerivation {
  pname = "hecate-shell";
  version = "0-unstable-${rev}";

  src = lib.fileset.toSource {
    root = ../shell/end4-pC;
    fileset = lib.fileset.difference ../shell/end4-pC ../shell/end4-pC/screenshots;
  };

  nativeBuildInputs = [
    makeWrapper
    qt6.wrapQtAppsHook
  ];
  buildInputs = qtModules;

  dontConfigure = true;
  dontBuild = true;
  # The Python scripts use `#!/usr/bin/env -S /bin/sh -c "source …/activate && exec python"`,
  # which patchShebangs can't handle; `/usr/bin/env` exists on NixOS anyway.
  dontPatchShebangs = true;

  installPhase = ''
    runHook preInstall

    mkdir -p $out/share/hecate-shell $out/bin
    cp -r . $out/share/hecate-shell/end4-pC
    config=$out/share/hecate-shell/end4-pC

    # QuickShell with everything the shell needs. wrapQtAppsHook adds the Qt modules.
    makeWrapper ${quickshell}/bin/qs $out/bin/qs \
      --prefix PATH : ${lib.makeBinPath runtimeTools} \
      --prefix XDG_DATA_DIRS : ${gsettings-desktop-schemas}/share/gsettings-schemas/${gsettings-desktop-schemas.name} \
      --set ILLOGICAL_IMPULSE_VIRTUAL_ENV ${venv} \
      --set HECATE_SHELL_NIX 1

    # Run the shell. The shell calls itself as `qs -c end4-pC`, so prefer that name when the
    # config is linked into ~/.config/quickshell (the Home Manager module does), else the store.
    cat > $out/bin/hecate-shell <<EOF
    #!${stdenv.shell}
    if [ -e "\''${XDG_CONFIG_HOME:-\$HOME/.config}/quickshell/end4-pC/shell.qml" ]; then
      exec $out/bin/qs -c end4-pC "\$@"
    fi
    exec $out/bin/qs -p $config "\$@"
    EOF

    # Call the shell's IPC, e.g. `hecate-shell-ipc shortcut press searchToggleRelease`.
    # One shell per user: don't filter by display, so it works from a TTY or ssh (no
    # WAYLAND_DISPLAY) as well as from hecate's binds.
    cat > $out/bin/hecate-shell-ipc <<EOF
    #!${stdenv.shell}
    if [ -e "\''${XDG_CONFIG_HOME:-\$HOME/.config}/quickshell/end4-pC/shell.qml" ]; then
      exec $out/bin/qs -c end4-pC ipc --any-display call "\$@"
    fi
    exec $out/bin/qs -p $config ipc --any-display call "\$@"
    EOF
    # xdg-desktop-portal-wlr's screen sharing chooser (chooser_type=simple): open the shell's
    # picker and print its answer ("Monitor: DP-1", "Window: hecate-…", nothing to decline).
    cat > $out/bin/hecate-share-picker <<EOF
    #!${stdenv.shell}
    dir=\$(${pkgs.coreutils}/bin/mktemp -d) || exit 1
    trap '${pkgs.coreutils}/bin/rm -rf "\$dir"' EXIT
    ${pkgs.coreutils}/bin/mkfifo "\$dir/answer" || exit 1
    $out/bin/hecate-shell-ipc sharePicker pick "\$dir/answer" >/dev/null || exit 1
    ${pkgs.coreutils}/bin/timeout 600 ${pkgs.coreutils}/bin/cat "\$dir/answer"
    EOF
    chmod +x $out/bin/hecate-shell $out/bin/hecate-shell-ipc $out/bin/hecate-share-picker

    runHook postInstall
  '';

  # Only wrap QuickShell, not the launcher scripts.
  dontWrapQtApps = true;
  postFixup = ''
    wrapQtApp $out/bin/qs
  '';

  passthru = {
    inherit pythonEnv venv;
    config = "share/hecate-shell/end4-pC";
  };

  meta = {
    description = "end-4's illogical-impulse shell (end4-pC) for hecate";
    homepage = "https://github.com/pctrade/end4-pC";
    license = lib.licenses.gpl3Only;
    mainProgram = "hecate-shell";
    platforms = lib.platforms.linux;
  };
}
