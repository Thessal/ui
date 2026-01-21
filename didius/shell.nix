{ pkgs ? import <nixpkgs> {} }:

let
  python = pkgs.python313;
  
  didius_oms = python.pkgs.buildPythonPackage {
    pname = "didius_oms";
    version = "0.1.0";
    format = "wheel";
    src = ./target/wheels/didius_oms-0.1.0-cp313-cp313-linux_x86_64.whl;
    doCheck = false;
    
    # Ensure shared libraries are found (common on NixOS)
    nativeBuildInputs = [
      pkgs.autoPatchelfHook
    ];
    
    buildInputs = [
      pkgs.openssl
      pkgs.stdenv.cc.cc.lib
    ];
  };

in pkgs.mkShell {
  packages = [
    (python.withPackages (ps: [ didius_oms ]))
    pkgs.maturin # useful to have around
    pkgs.cargo
    pkgs.rustc
  ];

  postBuild = ''
    echo "Dont forget to maturin build"
  '';  
  RUST_LOG = "info";
}
