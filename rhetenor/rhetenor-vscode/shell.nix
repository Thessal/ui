{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    nodejs_20
    nodePackages.npm
    vsce
    nodePackages.typescript
    nodePackages.typescript-language-server
    antigravity
  ];

  shellHook = ''
    echo "Rhetenor VS Code Extension Development Environment"
    echo "Node.js version: $(node --version)"
    echo "npm version: $(npm --version)"
    echo "Package the extension: vsce package"
    echo "Install the extension: code --install-extension <path-to-vsix>"
  '';
}
