{ ... }:
{
  flake.overlays = {
    default =
      final: prev:
      let
        atticPkgs = final.callPackage ../crane.nix { };
      in
      {
        inherit (atticPkgs)
          attic
          attic-client
          attic-server
          ;
      };
  };
}
