{
  lib,
  craneLib,
  common,
  mesa,
  libGL,
  libxkbcommon,
  wayland,
  runCommand,
  python3,
  nodejs,
  qt6,
}:
let
  shell = ../shell/end4-pC;
  # The shell's own scripts and logic, which only need an interpreter.
  scriptCheck =
    name: nativeBuildInputs: script:
    runCommand "hecate-shell-${name}" { inherit nativeBuildInputs; } ''
      export HOME=$TMPDIR
      cp -r ${shell} shell
      chmod -R u+w shell
      cd shell
      patchShebangs --build scripts/claude
      ${script}
      touch $out
    '';
in
{
  clippy = craneLib.cargoClippy (
    common.commonArgs
    // {
      inherit (common) cargoArtifacts;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings -A dead_code";
    }
  );

  fmt = craneLib.cargoFmt { inherit (common) src; };

  test = craneLib.cargoTest (
    common.commonArgs
    // {
      inherit (common) cargoArtifacts;
      # Software EGL for the headless render tests.
      __EGL_VENDOR_LIBRARY_FILENAMES = "${mesa}/share/glvnd/egl_vendor.d/50_mesa.json";
      LIBGL_ALWAYS_SOFTWARE = "1";
      GALLIUM_DRIVER = "llvmpipe";
      LD_LIBRARY_PATH = lib.makeLibraryPath [
        libGL
        libxkbcommon
        wayland
      ];
      HECATE_NO_SESSION = "1";
    }
  );

  # The Claude Code hook bridge (hook.py, bridge.py, sessions.py) against real sockets, with a
  # stand-in notify-send.
  shell-claude-bridge = scriptCheck "claude-bridge" [ python3 ] ''
    python3 -m unittest discover -v -s scripts/claude/tests -p 'test_*.py'
  '';

  # iCloud calendar parsing, and the wallpaper-template editor calls.
  shell-scripts = scriptCheck "scripts" [ python3 ] ''
    python3 -I scripts/apple/test_ical.py
    python3 -I scripts/colors/test_hecate_templates.py
  '';

  # The shell's pure-JS logic (ask panel parsing, Apple cards), without QML.
  shell-js = scriptCheck "js" [ nodejs ] ''
    node --test modules/ii/claude/tests/*.test.mjs
  '';

  # Static checks on every .qml/.js in the shell: syntax, structural mistakes, `qs.` imports
  # that name no directory. Needs no display and none of the shell's runtime inputs.
  shell-lint =
    runCommand "hecate-shell-lint"
      {
        nativeBuildInputs = [ python3 ];
      }
      ''
        python3 -I ${./tests/shell-lint.py} ${shell} ${qt6.qtdeclarative}/bin/qmllint ${nodejs}/bin/node
        touch $out
      '';
}
