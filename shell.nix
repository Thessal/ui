# morhpo and didius for testing
let
  pkgs = import <nixpkgs> {
    overlays = [
      (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
    ];
  };
  rustVersion = "1.91.1";
  myRust = pkgs.rust-bin.stable.${rustVersion}.default.override {
    extensions = [
      "rust-src" # for rust-analyzer
      "rust-analyzer"
    ];
  };

  rhetenor_didius = pkgs.fetchFromGitHub {
          owner = "Thessal";
          repo = "ui";
          rev = "main";
          sha256 = "sha256-X3abDj5gzsKGzTJgoftYo69GCvZOSM9qnpeGI9kBVNw=";
        };
  python_butterflow = import ./nixfiles/python.nix { pkgs=pkgs; }; # butterflow and morpho
  # python_butterflow = pkgs.python313
  pythonEnv_didius = import ./nixfiles/uv.nix { pkgs=pkgs; python=python_butterflow; projectRoot=(./didius); };
  pythonEnv_rhetenor = import ./nixfiles/uv.nix { pkgs=pkgs; python=python_butterflow; projectRoot=(./rhetenor); };
  python = python_butterflow.override {
    packageOverrides = self: super: {
      didius = self.buildPythonPackage rec {
        pname = "didius";
        version = "0.1.3";
        format = "pyproject";
        src = rhetenor_didius + "/didius";
        doCheck = false;
        cargoDeps = pkgs.rustPlatform.importCargoLock {
          lockFile = ./didius/Cargo.lock;
        };
        nativeBuildInputs = (with pkgs.rustPlatform; [cargoSetupHook maturinBuildHook]) ++ (with pkgs; [maturin pkg-config rustc cargo]);
        buildInputs = [ pkgs.openssl
          self.pyyaml
          python_butterflow.pkgs.morpho # didius don't have to depend on morpho
          python_butterflow.pkgs.butterflow
          ];
      };
      rhetenor = self.buildPythonPackage rec {
        pname = "rhetenor"; 
        version = "0.1.3"; 
        src = rhetenor_didius + "/rhetenor";
        buildInputs = with python.pkgs; [ hatchling pythonEnv_rhetenor ];
        format = "pyproject"; 
        nativeBuildInputs = [ self.hatchling ];
        propagatedBuildInputs = [ 
          self.ollama
          self.chromadb
          python_butterflow.pkgs.butterflow
        ];
        #pyproject = true;
      };
    };
  };
  rhetenorStatistics = pkgs.rustPlatform.buildRustPackage {
    pname = "rhetenor-statistics";
    version = "0.1.0";
    src = ./rhetenor/statistics;
    cargoLock = {
      lockFile = ./rhetenor/statistics/Cargo.lock;
    };
  };

  
in pkgs.mkShell {
  packages = [
    #dev
    rhetenorStatistics
    pkgs.antigravity
    pkgs.vsce
    pkgs.nodejs_20
    pkgs.nodePackages.npm
    pkgs.nodePackages.typescript
    pkgs.nodePackages.typescript-language-server

    #runtime
    pythonEnv_didius
    pythonEnv_rhetenor
    myRust
  ] ++ (with pkgs; [
    cargo rustc gcc rustfmt clippy rust-analyzer pkg-config
  ]) ++ (with python.pkgs; [ 
    didius rhetenor
    matplotlib pandas numpy ipython ipykernel jupyter pyyaml seaborn mplfinance boto3 zstandard
  ]);
  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}


