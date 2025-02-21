{
  description = "relm examples shell";

  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    cargo2nix.url = "github:cargo2nix/cargo2nix/release-0.11.0";
    rust-overlay.url = "github:oxalica/rust-overlay";
    nixpkgs.url = "nixpkgs/nixos-24.05";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-parts,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = nixpkgs.lib.systems.flakeExposed;
      perSystem =
        {
          self',
          pkgs,
          system,
          ...
        }:
        let
          rustVersion = "1.76.0";
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              inputs.cargo2nix.overlays.default
              (import inputs.rust-overlay)
            ];
          };
          runtimeDeps = with pkgs; [
            cairo
            gtk4
            atk
            glib
            gobject-introspection
            pango
            gdk-pixbuf
            graphene
            gtk4-layer-shell
          ];

        in
        {
          packages = rec {
            default = pkgs.dunst;
          };
          devShells.default = pkgs.mkShell rec {
            buildInputs =
              with pkgs;
              [
                pkg-config
              ]
              ++ runtimeDeps
              ++ [
                rust-analyzer-unwrapped
                (rust-bin.stable.${rustVersion}.default.override { extensions = [ "rust-src" ]; })
              ];
            LD_LIBRARY_PATH = "${nixpkgs.lib.makeLibraryPath buildInputs}";
          };
        };
    };
}
