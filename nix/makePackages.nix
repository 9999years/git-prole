{
  lib,
  newScope,
  inputs,
}:
lib.makeScope newScope (
  self:
    {inherit inputs;}
    // (lib.packagesFromDirectoryRecursive {
      directory = inputs.self + "/nix/packages";
      inherit (self) callPackage;
    })
)
