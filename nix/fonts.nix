# The haiiro fonts, for the whole system: IBM VGA 8x16 (regular, and the double-width 2x face
# as its bold), Jacquard 24, and GNU Unifont + Unifont Upper as the glyph fallback. The shell
# loads its own copies; this package is what lets every other program use the same faces, and
# what puts the fallback order into fontconfig. See shell/end4-pC/assets/fonts/haiiro/README.md.
#
#   fonts.packages = [ haiiro-fonts ];
#   fonts.fontconfig.confPackages = [ haiiro-fonts ];
#
# Under Home Manager, `home.packages = [ haiiro-fonts ]` with `fonts.fontconfig.enable = true`.
{
  lib,
  stdenvNoCC,
}:
stdenvNoCC.mkDerivation {
  pname = "haiiro-fonts";
  version = "1";

  src = ../shell/end4-pC/assets/fonts/haiiro;

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall

    install -Dm444 -t $out/share/fonts/truetype/haiiro *.ttf
    install -Dm444 -t $out/share/fonts/opentype/haiiro *.otf

    # The fallback order: QML and most toolkits name one family and let fontconfig find the
    # glyphs it lacks.
    install -Dm444 -t $out/etc/fonts/conf.d 45-haiiro.conf

    # WOFF2 copies for web pages. Kept out of share/fonts, where fontconfig would list every
    # face twice.
    install -Dm444 -t $out/share/haiiro-fonts/web web/*.woff2

    install -Dm444 -t $out/share/licenses/haiiro-fonts licenses/*
    install -Dm444 -t $out/share/doc/haiiro-fonts README.md

    runHook postInstall
  '';

  meta = {
    description = "The fonts hecate's haiiro shell is set in, with their fontconfig fallback order";
    homepage = "https://github.com/Kex1016/hecate";
    # IBM VGA 8x16: CC BY-SA 4.0. Jacquard 24: OFL 1.1. Unifont: OFL 1.1, or GPL 2+ with the
    # font embedding exception.
    license = with lib.licenses; [
      cc-by-sa-40
      ofl
    ];
    platforms = lib.platforms.all;
  };
}
