{
  self,
  lib,
  config,
  ...
}:

let

  evalCross =
    { system, pkgs }:
    config.allSystems.${system}.debug.extendModules {
      modules = [
        (
          { config, lib, ... }:
          {
            _module.args.pkgs = pkgs;
            _module.args.self' = lib.mkForce config;
          }
        )
      ];
    };
in
{
  config = {
    perSystem =
      {
        self',
        pkgs,
        system,
        ...
      }:
      let
        atticPkgs = pkgs.callPackage ../attic.nix { };
        atticPkgsStatic = pkgs.pkgsStatic.callPackage ../attic.nix { };
      in
      (lib.mkMerge [
        {
          _module.args = { };

          packages = {
            default = atticPkgs.attic;

            inherit (atticPkgs)
              attic
              attic-client
              attic-server
              ;

            attic-static = atticPkgsStatic.attic;
            attic-client-static = atticPkgsStatic.attic-client;
            attic-server-static = atticPkgsStatic.attic-server;

            attic-ci-installer = pkgs.callPackage ../ci-installer.nix {
              inherit self;
            };

            book = pkgs.callPackage ../book {
              attic = self'.packages.attic;
            };
          };
        }

        (lib.mkIf pkgs.stdenv.isLinux {
          packages = {
            attic-server-image = pkgs.dockerTools.streamLayeredImage {
              name = "attic-server";
              tag = "main";
              contents = [
                self'.packages.attic-server

                pkgs.busybox

                pkgs.dockerTools.fakeNss
              ];
              config = {
                Entrypoint = [ "/bin/atticd" ];
                Env = [
                  "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
                ];
              };
            };
          };
        })

        (lib.mkIf (system == "x86_64-linux") {
          packages = {
            attic-server-image-aarch64 =
              let
                eval = evalCross {
                  system = "aarch64-linux";
                  pkgs = pkgs.pkgsCross.aarch64-multiplatform;
                };

              in
              eval.config.packages.attic-server-image;
          };
        })
      ]);
  };
}
