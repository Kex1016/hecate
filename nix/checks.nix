{
  lib,
  craneLib,
  common,
  mesa,
  libGL,
  libxkbcommon,
  wayland,
}:
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
}
