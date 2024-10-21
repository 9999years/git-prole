# Installation

<a href="https://repology.org/project/git-prole/versions">
<img src="https://repology.org/badge/vertical-allrepos/git-prole.svg" alt="Packaging status">
</a>

## Nixpkgs

`git-prole` is [available in `nixpkgs` as `git-prole`][nixpkgs]:

```shell
nix-env -iA git-prole
nix profile install nixpkgs#git-prole
# Or add to your `/etc/nixos/configuration.nix`.
```

[nixpkgs]: https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/gi/git-prole/package.nix

## Statically-linked binaries

Statically-linked binaries for aarch64/x86_64 macOS/Linux can be downloaded
from the [GitHub releases][latest].

[latest]: https://github.com/9999years/git-prole/releases/latest

## Crates.io

The Rust crate can be downloaded from [crates.io][crate]:

```shell
cargo install git-prole
```

[crate]: https://crates.io/crates/git-prole
