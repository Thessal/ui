let
  pkgs = import <nixpkgs> { };
  python = import ../nixfiles/python.nix { pkgs=pkgs; };
  pythonEnv = import ../nixfiles/uv.nix { pkgs=pkgs; python=python; projectRoot=(./.); };

  didiusPackage = python.pkgs.buildPythonPackage {
    pname = "didius";
    version = "0.1.0";
    format = "pyproject";
    src = ./.;
    doCheck = false;
    
    cargoDeps = pkgs.rustPlatform.importCargoLock {
      lockFile = ./Cargo.lock;
    };
    nativeBuildInputs = (with pkgs.rustPlatform; [cargoSetupHook maturinBuildHook]) ++ (with pkgs; [maturin pkg-config rustc cargo]);
    buildInputs = [ pkgs.openssl python.pkgs.butterflow python.pkgs.morpho ];
  };

in pkgs.mkShell {
  packages = [
    pythonEnv
    didiusPackage
    pkgs.cargo # for dev 
    pkgs.rustc # for dev
  ] ++ (with python.pkgs; [
    matplotlib                                                                
    pandas                                                                    
    numpy                                                                     
    ipython
    jupyter
    #seaborn
    #mplfinance
    pyyaml
  ]);
}
