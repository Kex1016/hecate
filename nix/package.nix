{
  lib,
  craneLib,
  common,
  libGL,
  wayland,
  libxkbcommon,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
  xwayland-satellite,
}:
let
  # Loaded with dlopen at runtime. Kept in the RPATH rather than LD_LIBRARY_PATH so spawned
  # applications don't inherit it.
  dlopenLibs = [
    wayland
    libGL
    libxkbcommon
    libx11
    libxcursor
    libxi
    libxrandr
  ];
in
craneLib.buildPackage (
  common.commonArgs
  // {
    inherit (common) cargoArtifacts;
    # Tests run in the separate `checks.test` derivation (they need software EGL).
    doCheck = false;
    # The fixup phase shrinks RPATHs to what's linked; the dlopen libraries must stay.
    dontPatchELF = true;

    postInstall = ''
      patchelf --add-rpath ${lib.makeLibraryPath dlopenLibs} $out/bin/hecate
      wrapProgram $out/bin/hecate \
        --prefix PATH : ${lib.makeBinPath [ xwayland-satellite ]}

      install -Dm644 resources/hecate.desktop $out/share/wayland-sessions/hecate.desktop
      install -Dm644 resources/hecate-portals.conf $out/share/xdg-desktop-portal/hecate-portals.conf
      install -Dm644 resources/hecate-session.target $out/share/systemd/user/hecate-session.target

      mkdir -p $out/share/bash-completion/completions $out/share/zsh/site-functions $out/share/fish/vendor_completions.d
      $out/bin/hecate completions bash > $out/share/bash-completion/completions/hecate
      $out/bin/hecate completions zsh > $out/share/zsh/site-functions/_hecate
      $out/bin/hecate completions fish > $out/share/fish/vendor_completions.d/hecate.fish

      # For editors (taplo, VS Code) validating ~/.config/hecate/config.toml.
      mkdir -p $out/share/hecate
      $out/bin/hecate schema > $out/share/hecate/config.schema.json
    '';

    passthru = {
      providedSessions = [ "hecate" ];
    };

    meta = {
      description = "A stacking Wayland compositor";
      homepage = "https://github.com/kex1016/wl-hex";
      license = lib.licenses.gpl3Plus;
      mainProgram = "hecate";
      platforms = lib.platforms.linux;
    };
  }
)
