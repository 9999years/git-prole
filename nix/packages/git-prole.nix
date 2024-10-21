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
  mdbook,
  installShellFiles,
  pkg-config,
  openssl,
  bash,
  git,
}:
let
  src = lib.cleanSourceWith {
    src = craneLib.path inputs.self.outPath;
    # Keep test data.
    filter = path: type: lib.hasInfix "/data" path || (craneLib.filterCargoSources path type);
  };

  commonArgs' =
    (craneLib.crateNameFromCargoToml {
      cargoToml = "${inputs.self}/Cargo.toml";
    })
    // {
      inherit src;

      cargoBuildCommand = "cargoWithProfile build --all";
      cargoCheckExtraArgs = "--all";
      cargoTestExtraArgs = "--all";

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

      # Ensure that binaries are statically linked.
      postPhases = "ensureStaticPhase";
      doEnsureStatic = true;
      ensureStaticPhase =
        let
          ldd = if stdenv.isDarwin then "otool -L" else "ldd";
        in
        ''
          if [[ "$doEnsureStatic" = 1 && -d "$out/bin" ]]; then
            for installedBinary in $(find $out/bin/ -type f); do
              echo "Checking that $installedBinary is statically linked"
              # The first line of output is the binary itself, stored in
              # `/nix/store`, so we skip that with `tail`.
              if ${ldd} "$installedBinary" | tail -n +2 | grep --quiet /nix/store; then
                ${ldd} "$installedBinary"
                echo "Output binary $installedBinary isn't statically linked!"
                exit 1
              fi
            done
          fi
        '';
    }
    // (lib.optionalAttrs (stdenv.targetPlatform.isLinux && stdenv.targetPlatform.isx86_64) {
      # Make sure we don't link with GNU libc so we can produce a static executable.
      CARGO_BUILD_TARGET = "x86_64-unknown-linux-musl";
    })
    // (lib.optionalAttrs (stdenv.targetPlatform.isLinux && stdenv.targetPlatform.isAarch64) {
      # Make sure we don't link with GNU libc so we can produce a static executable.
      CARGO_BUILD_TARGET = "aarch64-unknown-linux-musl";
      CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER = "${stdenv.cc.targetPrefix}cc";
    });

  # Build *just* the cargo dependencies, so we can reuse
  # all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs';

  commonArgs = commonArgs' // {
    inherit cargoArtifacts;
  };

  releaseArgs = commonArgs // {
    # Don't run tests; we'll do that in a separate derivation.
    # This will allow people to install and depend on `git-prole`
    # without downloading a half dozen different versions of GHC.
    doCheck = false;

    # Only build `git-prole`, not the test harness.
    cargoBuildCommand = "cargoWithProfile build";

    passthru = {
      inherit
        checks
        devShell
        user-manual
        user-manual-tar-xz
        ;
    };
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
    releaseArgs
    // {
      pnameSuffix = "-man";

      cargoExtraArgs = "${releaseArgs.cargoExtraArgs or ""} --locked --features clap_mangen";

      nativeBuildInputs = releaseArgs.nativeBuildInputs ++ [ installShellFiles ];

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

  user-manual = stdenv.mkDerivation {
    pname = "git-prole-user-manual";
    inherit (commonArgs) version;

    phases = [
      "unpackPhase"
      "buildPhase"
      "installPhase"
    ];

    src = inputs.self;
    sourceRoot = "source/docs";

    nativeBuildInputs = [ mdbook ];

    buildPhase = ''
      mdbook build
    '';

    installPhase = ''
      mkdir -p "$out/share/git-prole"
      cp -r book "$out/share/git-prole/html-manual"
    '';
  };

  user-manual-tar-xz = stdenv.mkDerivation {
    name = "git-prole-user-manual-${commonArgs.version}.tar.xz";

    src = user-manual;

    phases = [
      "unpackPhase"
      "installPhase"
    ];

    installPhase = ''
      mv share/git-prole/html-manual git-prole-user-manual

      tar --create \
        --verbose \
        --auto-compress \
        --file "$out" \
        git-prole-user-manual
    '';
  };

  checks = {
    inherit
      user-manual
      user-manual-tar-xz
      ;

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
in
# Build the actual crate itself, reusing the dependency
# artifacts from above.
craneLib.buildPackage (
  releaseArgs
  // {
    postInstall =
      (commonArgs.postInstall or "")
      + ''
        cp -r ${git-prole-man}/share $out/share
        # For some reason this is needed to strip references:
        #     stripping references to cargoVendorDir from share/man/man1/git-prole.1.gz
        #     sed: couldn't open temporary file share/man/man1/sedwVs75O: Permission denied
        chmod -R +w $out/share
      '';
  }
)
