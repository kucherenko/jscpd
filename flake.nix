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
          sha256 = "sha256-SBKjxhC6zHTu0SyJwxLlQHItzMzYZ71VCWQC2hOzpRY=";
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = pkgs.lib.cleanSourceWith {
          src = ./rust;
          filter = path: type:
            (craneLib.filterCargoSources path type) ||
            (pkgs.lib.hasSuffix ".html" path && pkgs.lib.hasInfix "/templates/" path);
        };

        commonArgs = {
          inherit src;
          pname = "jscpd";
          version = (craneLib.crateNameFromCargoToml { cargoToml = ./rust/crates/cpd/Cargo.toml; }).version;
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.apple-sdk
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

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