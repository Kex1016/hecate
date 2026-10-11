{
  lib,
  craneLib,
  rev,
  pkg-config,
  makeWrapper,
  libinput,
  libxkbcommon,
  libgbm,
  libdrm,
  libGL,
  udev,
  seatd,
  wayland,
  pixman,
  libdisplay-info,
  xwayland-satellite,
}:
let
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
    ];
  };

  commonArgs = {
    inherit src;
    pname = "hecate";
    strictDeps = true;
    nativeBuildInputs = [
      pkg-config
      makeWrapper
    ];
    buildInputs = [
      libinput
      libxkbcommon
      libgbm
      libdrm
      libGL
      udev
      seatd
      wayland
      pixman
      libdisplay-info
    ];
    HECATE_GIT_REV = rev;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;

    postInstall = ''
      wrapProgram $out/bin/hecate \
        --prefix PATH : ${lib.makeBinPath [ xwayland-satellite ]} \
        --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ libGL ]}
    '';

    passthru = {
      inherit cargoArtifacts;
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
