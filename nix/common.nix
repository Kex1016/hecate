# Shared crane arguments for the package and the flake checks.
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
}:
rec {
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../rustfmt.toml
      ../crates
    ];
  };

  commonArgs = {
    inherit src;
    pname = "hecate";
    version = "0.1.0";
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
}
