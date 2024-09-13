{
  lib,
  writeShellApplication,
  git-prole,
}:
writeShellApplication {
  name = "get-crate-version";

  text = ''
    echo ${lib.escapeShellArg git-prole.version}
  '';
}
