{
  description = "wokesy, the Zulip bot that manages who is on call in PagerDuty, as a container image";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    dull-nix.url = "github:dull-ca/nix";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, dull-nix }:
    let
      pkgsFor = system: import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default dull-nix.overlays.default ];
      };

      releaseTools = flake-utils.lib.eachDefaultSystem (system:
        let pkgs = pkgsFor system; in
        {
          packages = {
            release = pkgs.mkReleaseCommand {
              repositoryUrl = "https://github.com/lakinwecker/wokesy";
              hooks = ./ci/release-hooks.sh;
              releaseWorkflow = "release.yml";
            };
            release-guards = pkgs.releaseGuards;
          };
        });

      image = flake-utils.lib.eachSystem [ "x86_64-linux" ] (system:
      let
        pkgs = pkgsFor system;
        inherit (pkgs) lib;

        toolchain = pkgs.rust-bin.stable.latest.default;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = toolchain;
          rustc = toolchain;
        };
        version = (lib.importTOML ./Cargo.toml).workspace.package.version;
        httpPort = 8080;

        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [ ./Cargo.toml ./Cargo.lock ./clippy.toml ./src ];
        };

        wokesy = rustPlatform.buildRustPackage {
          pname = "wokesy";
          inherit version src;
          cargoLock.lockFile = ./Cargo.lock;
          doCheck = false;
          meta.mainProgram = "wokesy";
        };

        container = pkgs.dockerTools.buildLayeredImage {
          name = "wokesy";
          tag = "latest";
          contents = [ pkgs.dockerTools.fakeNss pkgs.dockerTools.caCertificates wokesy ];
          extraCommands = "mkdir -m 1777 tmp";
          config = {
            Cmd = [ "wokesy" ];
            User = "65534:65534";
            ExposedPorts."${toString httpPort}/tcp" = { };
            Env = [
              "HOME=/tmp"
              "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt"
              "WOKESY_HTTP__HOST=0.0.0.0"
              "WOKESY_HTTP__PORT=${toString httpPort}"
            ];
            Labels = {
              "org.opencontainers.image.source" = "https://github.com/lakinwecker/wokesy";
              "org.opencontainers.image.version" = version;
            };
          };
        };

        formatting = pkgs.runCommand "wokesy-formatting" { nativeBuildInputs = [ toolchain ]; } ''
          rustfmt --check --edition 2024 ${src}/src/main.rs ${src}/src/lib.rs
          touch $out
        '';
      in
      {
        checks = {
          inherit wokesy container formatting;
          release-guards-hold = pkgs.releaseGuardsTest;
        };

        packages = {
          inherit wokesy container;
          default = container;
        };
      });
    in
    nixpkgs.lib.recursiveUpdate releaseTools image;
}
