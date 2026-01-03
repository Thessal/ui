{ pkgs } : 
pkgs.python313.override {
  packageOverrides = self: super: {
    butterflow = self.buildPythonPackage rec {
      pname = "butterflow";
      version = "0.1.1";

      src = pkgs.fetchFromGitHub {
        owner = "Thessal";
        repo = "butterflow";
        rev = "main";
        sha256 = "sha256-CPnqm3Fggl1uYWUyXfWECN0A6cKGIzqzeXbroX5RcUU=";
      };

      nativeBuildInputs = [ self.hatchling ];

      propagatedBuildInputs = [ 
        self.numpy
        self.scipy
        self.pytest
      ];

      pyproject = true;
    };
    morpho = self.buildPythonPackage rec {
      pname = "morpho";
      version = "0.1.0";

      src = pkgs.fetchFromGitHub {
        owner = "Thessal";
        repo = "transpiler";
        rev = "main";
        sha256 = "sha256-UNuNWX4HY0pdZ6yY1rA8fwSjGgHOKS/FQfVGJIbJZgc=";
      };

      nativeBuildInputs = [ self.hatchling ];

      propagatedBuildInputs = [ 
        self.ollama
        self.chromadb
        self.butterflow
      ];

      pyproject = true;
    };
  };
}
