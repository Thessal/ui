let
  pkgs = import <nixpkgs> { };
  python = import ../nixfiles/python.nix { pkgs=pkgs; };
  pythonEnv = import ../nixfiles/uv.nix { pkgs=pkgs; python=python; projectRoot=(./.); };

  rhetenorPackage = python.pkgs.buildPythonPackage rec {
    pname = "rhetenor"; 
    version = "0.1.3"; 
    src = ./.; 

    buildInputs = with python.pkgs; [
      hatchling
      pythonEnv
    ];
    
    format = "pyproject"; 
  };

  rhetenorStatistics = pkgs.rustPlatform.buildRustPackage {
    pname = "rhetenor-statistics";
    version = "0.1.3";
    src = ./statistics;
    cargoLock = {
      lockFile = ./statistics/Cargo.lock;
    };
  };

in pkgs.mkShell { 
  packages = [ 
    pkgs.antigravity 

    # Plugin development
    pkgs.vsce 
    pkgs.nodejs_20
    pkgs.nodePackages.npm
    pkgs.nodePackages.typescript
    pkgs.nodePackages.typescript-language-server

    # rust 
    rhetenorStatistics
    pkgs.cargo
    pkgs.rustc

    # python 
    pythonEnv
    rhetenorPackage
    pkgs.uv
    ] ++ (with python.pkgs; [
      matplotlib                                                                
      pandas                                                                    
      numpy                                                                     
      ipython
      jupyter
      seaborn
      mplfinance
      
      # Dependencies needed for data loader
      boto3
      zstandard
      pyyaml

      # Chart 
      plotly
  ]); 
}
