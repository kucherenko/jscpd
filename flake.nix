{
  description = "Copy/paste detector — fast Rust-based CLI for code duplication detection";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane = {
      url = "github:ipetkov/crane";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, fenix, crane, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        rustToolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust/rust-toolchain.toml;
          # Hash of the versioned channel manifest (channel-rust-<x.y.z>.toml);
          # the toolchain file must pin an exact patch version or this drifts.
          sha256 = "sha256-A1abGIbOtcBSdrUMhDGrER3pRM1hQP4fp9gh3Y4PKc8=";
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # All of rust/, not only the Cargo sources: the crates include other
        # files at compile time (README.md, page.html, frameworks.yaml, the
        # report templates), and a list of them goes stale.
        src = pkgs.lib.cleanSource ./rust;

        commonArgs = {
          inherit src;
          pname = "jscpd";
          version = (craneLib.crateNameFromCargoToml { cargoToml = ./rust/crates/cpd/Cargo.toml; }).version;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [
            pkgs.libiconv
            pkgs.apple-sdk
          ];
        };

        # crane builds the dependencies against a copy of the workspace in
        # which every local crate is an empty stub. patches/paste stands in
        # for paste from crates.io (see Cargo.toml), so it keeps its real code
        # there, or pulp and gemm find no paste! macro.
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          extraDummyScript = ''
            rm -rf $out/patches
            cp -r ${./rust/patches} $out/patches
            chmod -R +w $out/patches
          '';
        });

        jscpd = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          doCheck = false;
          meta = {
            description = "Copy/paste detector for programming source code";
            homepage = "https://jscpd.dev";
            license = pkgs.lib.licenses.mit;
            mainProgram = "jscpd";
          };
        });
      in
      {
        packages = {
          inherit jscpd;
          default = jscpd;
        };

        checks = {
          jscpd-tests = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            cargoExtraArgs = "--lib -p cpd-core -p cpd-tokenizer -p cpd-reporter";
          });
        };

        devShells.default = craneLib.devShell (commonArgs // {
          packages = with pkgs; [
            cargo-nextest
            cargo-deny
            pkg-config
          ];
        });
      }
    );
}