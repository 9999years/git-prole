{
  lib,
  stdenv,
  buildPackages,
  pkgsStatic,
  darwin,
  craneLib,
  inputs,
  rustPlatform,
  rust-analyzer,
  cargo-release,
  installShellFiles,
  pkg-config,
  openssl,
  bash,
  git,
}:
let
  src = lib.cleanSourceWith {
    src = craneLib.path ../../.;
    # Keep test data.
    filter = path: type: lib.hasInfix "/data" path || (craneLib.filterCargoSources path type);
  };

  commonArgs' = {
    inherit src;

    nativeBuildInputs =
      lib.optionals stdenv.isLinux [
        pkg-config
        openssl
      ]
      ++ lib.optionals stdenv.isDarwin [
        pkgsStatic.libiconv
        darwin.apple_sdk.frameworks.CoreServices
        darwin.apple_sdk.frameworks.SystemConfiguration
      ];

    OPENSSL_NO_VENDOR = true;
  };

  # Build *just* the cargo dependencies, so we can reuse
  # all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs';

  commonArgs = commonArgs' // {
    inherit cargoArtifacts;
  };

  checks = {
    git-prole-nextest = craneLib.cargoNextest (
      commonArgs
      // {
        nativeBuildInputs = commonArgs.nativeBuildInputs ++ [
          bash
          git
        ];
        NEXTEST_HIDE_PROGRESS_BAR = "true";
      }
    );
    git-prole-doctest = craneLib.cargoDocTest commonArgs;
    git-prole-clippy = craneLib.cargoClippy (
      commonArgs
      // {
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
      }
    );
    git-prole-rustdoc = craneLib.cargoDoc (
      commonArgs
      // {
        cargoDocExtraArgs = "--document-private-items";
        RUSTDOCFLAGS = "-D warnings";
      }
    );
    git-prole-fmt = craneLib.cargoFmt commonArgs;
    git-prole-audit = craneLib.cargoAudit (
      commonArgs
      // {
        inherit (inputs) advisory-db;
      }
    );
  };

  devShell = craneLib.devShell {
    inherit checks;

    # Make rust-analyzer work
    RUST_SRC_PATH = rustPlatform.rustLibSrc;

    # Extra development tools (cargo and rustc are included by default).
    packages = [
      rust-analyzer
      cargo-release
    ];
  };

  can-run-git-prole = stdenv.hostPlatform.emulatorAvailable buildPackages;
  git-prole = "${stdenv.hostPlatform.emulator buildPackages} $out/bin/git-prole";

  git-prole-man = craneLib.buildPackage (
    commonArgs
    // {
      cargoExtraArgs = "${commonArgs.cargoExtraArgs or ""} --features clap_mangen";

      nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ installShellFiles ];

      doCheck = false;

      postInstall =
        (commonArgs.postInstall or "")
        + lib.optionalString can-run-git-prole ''
          manpages=$(mktemp -d)
          ${git-prole} manpages "$manpages"
          for manpage in "$manpages"/*; do
            installManPage "$manpage"
          done

          installShellCompletion --cmd git-prole \
            --bash <(${git-prole} completions bash) \
            --fish <(${git-prole} completions fish) \
            --zsh <(${git-prole} completions zsh)

          rm -rf "$out/bin"
        '';
    }
  );
in
# Build the actual crate itself, reusing the dependency
# artifacts from above.
craneLib.buildPackage (
  commonArgs
  // {
    # Don't run tests; we'll do that in a separate derivation.
    doCheck = false;

    postInstall =
      (commonArgs.postInstall or "")
      + ''
        cp -r ${git-prole-man}/share $out/share
        # For some reason this is needed to strip references:
        #     stripping references to cargoVendorDir from share/man/man1/git-prole.1.gz
        #     sed: couldn't open temporary file share/man/man1/sedwVs75O: Permission denied
        chmod -R +w $out/share
      '';

    passthru = {
      inherit
        checks
        devShell
        commonArgs
        craneLib
        ;
    };
  }
)
