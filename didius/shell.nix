let
  pkgs = import <nixpkgs> { };
  python = import ../nixfiles/python.nix { pkgs=pkgs; };
  pythonEnv = import ../nixfiles/uv.nix { pkgs=pkgs; python=python; projectRoot=(./.); };

  didiusPackage = python.pkgs.buildPythonPackage rec {
    pname = "didius"; 
    version = "0.1.2"; 
    src = ./.; 

    buildInputs = with python.pkgs; [
      hatchling
      pythonEnv
    ];
    
    format = "pyproject"; 
    doCheck = false; 
    doInstallCheck = false;
    dontCheck = true;
  };

in pkgs.mkShell { 
  packages = [ 
    pythonEnv
    didiusPackage
    pkgs.uv
    ]; 
}

