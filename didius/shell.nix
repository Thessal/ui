let
  pkgs = import <nixpkgs> {
    overlays = [
      (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  };
  python = import ../nixfiles/python.nix { pkgs=pkgs; };
  pythonEnv = import ../nixfiles/uv.nix { pkgs=pkgs; python=python; projectRoot=(./.); };

  didiusPackage = python.pkgs.buildPythonPackage {
    pname = "didius";
    version = "0.1.3";
    format = "pyproject";
    src = ./.;
    doCheck = false;
    
    cargoDeps = pkgs.rustPlatform.importCargoLock {
      lockFile = ./Cargo.lock;
    };
    nativeBuildInputs = (with pkgs.rustPlatform; [cargoSetupHook maturinBuildHook]) ++ (with pkgs; [maturin pkg-config rustc cargo]);
    buildInputs = [ pkgs.openssl python.pkgs.butterflow python.pkgs.morpho ];
  };

  rustVersion = "1.91.1";
  myRust = pkgs.rust-bin.stable.${rustVersion}.default.override {
    extensions = [
      "rust-src" # for rust-analyzer
      "rust-analyzer"
    ];
  };

in pkgs.mkShell {
  packages = [
    pkgs.antigravity
    # pythonEnv
    didiusPackage
    myRust
  ] ++ (with pkgs; [
    cargo rustc gcc rustfmt clippy rust-analyzer pkg-config maturin
    # rustc 1.91.1 (ed61e7d7e 2025-11-07) (built from a source tarball)
  ]) ++ (with python.pkgs; [
    matplotlib                                                                
    pandas                                                                    
    numpy                                                                     
    ipython
    ipykernel
    jupyter
    #seaborn
    #mplfinance
    pyyaml
  ]);
  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}
