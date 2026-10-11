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
      # A test checks the base binds against their docs.
      ../docs/keybinds.md
    ];
  };

  commonArgs = {
    inherit src;
    pname = "hecate";
    # From the workspace, so there is one place to bump it.
    version = (lib.importTOML ../Cargo.toml).workspace.package.version;
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
