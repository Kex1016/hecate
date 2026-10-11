{
  lib,
  craneLib,
  common,
  libGL,
  xwayland-satellite,
}:
craneLib.buildPackage (
  common.commonArgs
  // {
    inherit (common) cargoArtifacts;
    # Tests run in the separate `checks.test` derivation (they need software EGL).
    doCheck = false;

    postInstall = ''
      wrapProgram $out/bin/hecate \
        --prefix PATH : ${lib.makeBinPath [ xwayland-satellite ]} \
        --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ libGL ]}

      install -Dm644 resources/hecate.desktop $out/share/wayland-sessions/hecate.desktop
      install -Dm644 resources/hecate-portals.conf $out/share/xdg-desktop-portal/hecate-portals.conf
      install -Dm644 resources/hecate-session.target $out/share/systemd/user/hecate-session.target

      mkdir -p $out/share/bash-completion/completions $out/share/zsh/site-functions $out/share/fish/vendor_completions.d
      $out/bin/hecate completions bash > $out/share/bash-completion/completions/hecate
      $out/bin/hecate completions zsh > $out/share/zsh/site-functions/_hecate
      $out/bin/hecate completions fish > $out/share/fish/vendor_completions.d/hecate.fish
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
