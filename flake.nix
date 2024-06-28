{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ fenix.overlays.default ];
        };

        toolchain = pkgs.fenix.stable;

        flakePkgs = {
          #markout = pkgs.callPackage ./. { inherit toolchain; }; 
        };
      in rec {
        packages = flake-utils.lib.flattenTree flakePkgs;

        devShell = pkgs.mkShell {
          inputsFrom = with packages;
            [
              #markout 
            ];
          packages = (with pkgs; [
            cargo-watch
            cargo-deny
            cargo-edit
            corepack
            nodejs_20
            sqlx-cli
            go
          ]) ++ (with toolchain; [ cargo rustc rustfmt clippy ]);
        };
      });
}
