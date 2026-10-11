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
  libdisplay-info_0_3,
}:
rec {
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../rustfmt.toml
      ../crates
      ../resources
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
      libdisplay-info_0_3
    ];
    HECATE_GIT_REV = rev;
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
}
