{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        packages.default = naersk-lib.buildPackage {
          src = ./.;

          postInstall = ''
            OUT_DIR="$(find target/release/build -type d -path "target/release/build/http-hammer-*/out")"
            if [ "$(echo "$OUT_DIR" | wc -l)" -ne 1 ]; then
              echo "error: Too many out dirs generated" >&2
              return 1
            fi

            install -Dm644 "$OUT_DIR/completions/_http-hammer" "$out/share/zsh/site-functions/_http-hammer"
            install -Dm644 "$OUT_DIR/completions/http-hammer.bash" "$out/share/bash-completion/completions/http-hammer"
            install -Dm644 "$OUT_DIR/completions/http-hammer.fish" "$out/share/fish/vendor_completions.d/http-hammer.fish"
          '';

          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [ openssl ];
        };
        overlays.default = prev: final: {
          http-hammer = self.packages."${system}".default;
        };
        devShell = with pkgs; mkShell {
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy openssl pkg-config ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
        };
      });
}
