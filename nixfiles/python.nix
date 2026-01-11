{ pkgs } : 
pkgs.python313.override {
  packageOverrides = self: super: {
    butterflow = self.buildPythonPackage rec {
      pname = "butterflow";
      version = "0.1.2";

      src = pkgs.fetchFromGitHub {
        owner = "Thessal";
        repo = "butterflow";
        rev = "main";
        sha256 = "sha256-TG0AIdsnwBrh6UsmWJfJDqkdvvEbAjXqARfzHKhoRQA=";
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
      version = "0.1.2";

      src = pkgs.fetchFromGitHub {
        owner = "Thessal";
        repo = "transpiler";
        rev = "main";
        sha256 = "sha256-Ci+3k1By4LCFFmGZRaq4NFCW6rsAH1LTWbgfqP6hVX0=";
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
