{
  stdenv,
  lib,
  rustPlatform,
  cmake,
  git,
  pkg-config,
  installShellFiles,
  jq,

  nix,
  boost,
  libarchive,
}:

let
  attic = rustPlatform.buildRustPackage {
    pname = "attic";
    version = "0.1.0";
    src = lib.fileset.toSource {
      root = ./.;
      fileset = lib.fileset.unions [
        ./Cargo.toml
        ./Cargo.lock
        ./attic
        ./server
        ./client
        ./token
      ];
    };

    nativeBuildInputs = [
      pkg-config
      cmake
      git
      installShellFiles
      rustPlatform.bindgenHook
    ];

    buildInputs = [
      nix
      boost
      libarchive
    ];

    cargoLock.lockFile = ./Cargo.lock;

    checkFlags = [
      "--skip"
      "nix_store"
    ];

    postInstall = lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
      if [[ -f $out/bin/attic ]]; then
        installShellCompletion --cmd attic \
          --bash <($out/bin/attic gen-completions bash) \
          --zsh <($out/bin/attic gen-completions zsh) \
          --fish <($out/bin/attic gen-completions fish)
      fi
    '';

    env = {
      ATTIC_DISTRIBUTOR = "pineapplehunter";
    };

    meta = with lib; {
      description = "Multi-tenant Nix binary cache system";
      homepage = "https://github.com/zhaofengli/attic";
      license = licenses.asl20;
      maintainers = with maintainers; [
        zhaofengli
        pineapplehunter
      ];
      platforms = platforms.linux ++ platforms.darwin;
    };

    passthru = {
      inherit nix;
    };
  };

  attic-client = attic.overrideAttrs {
    pname = "attic-client";
    buildAndTestSubdir = "client";
    meta.mainProgram = "attic";
  };

  attic-server = attic.overrideAttrs {
    pname = "attic-server";
    buildAndTestSubdir = "server";
  };

in
{
  inherit
    attic
    attic-client
    attic-server
    ;
}
