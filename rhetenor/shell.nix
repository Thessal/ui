let
  pkgs = import <nixpkgs> { };
  python = import ../nixfiles/python.nix { pkgs=pkgs; };
  pythonEnv = import ../nixfiles/uv.nix { pkgs=pkgs; python=python; projectRoot=(./.); };

  rhetenorPackage = python.pkgs.buildPythonPackage rec {
    pname = "rhetenor"; 
    version = "0.1.2"; 
    src = ./.; 

    buildInputs = with python.pkgs; [
      hatchling
      pythonEnv
    ];
    
    format = "pyproject"; 
  };

in pkgs.mkShell { 
  packages = [ 
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
