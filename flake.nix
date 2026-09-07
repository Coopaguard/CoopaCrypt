{
  # Paquets Nix pour CoopaCrypt.
  #
  # Nix ne fait pas partie des cibles produites par la release : les paquets
  # `.deb`, `.rpm` et l'AppImage supposent un système de fichiers classique,
  # ce que NixOS n'offre pas. Une dérivation est donc la façon propre d'y
  # installer l'application.
  #
  # Deux paquets :
  #   - `cli` : Rust pur, aucune dépendance système. Le plus simple à faire
  #     vivre, et suffisant pour un usage en ligne de commande.
  #   - `app` : l'interface graphique, qui a besoin de webkitgtk.
  #
  # Utilisation :
  #   nix run  github:Coopaguard/CoopaCrypt#cli -- --help
  #   nix build github:Coopaguard/CoopaCrypt#app
  #   nix develop            # environnement de développement complet

  description = "Carnet de notes chiffré : un seul fichier markdown, sans serveur ni compte";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Dépendances de compilation de l'interface graphique. Elles ne
        # concernent pas la CLI, qui n'a besoin de rien.
        guiBuildInputs = with pkgs; [
          webkitgtk_4_1
          gtk3
          libayatana-appindicator
          librsvg
          openssl
        ];

        guiNativeBuildInputs = with pkgs; [
          pkg-config
          wrapGAppsHook3
          nodejs_22
        ];
      in
      {
        packages = {
          default = self.packages.${system}.cli;

          # Outil en ligne de commande. Aucune dépendance système : la
          # dérivation se réduit à une compilation Rust.
          cli = pkgs.rustPlatform.buildRustPackage {
            pname = "coopacrypt-cli";
            version = "0.2.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            cargoBuildFlags = [ "-p" "coopacrypt-cli" ];
            # Les tests d'intégration sont marqués `#[ignore]` : ils ne sont
            # pas exécutés ici, et les tests unitaires suffisent à valider la
            # compilation.
            cargoTestFlags = [ "-p" "coopacrypt-cli" "-p" "coopacrypt-core" ];

            meta = with pkgs.lib; {
              inherit (self) description;
              homepage = "https://github.com/Coopaguard/CoopaCrypt";
              license = licenses.mit;
              mainProgram = "coopacrypt";
              platforms = platforms.unix;
            };
          };

          # Application de bureau.
          #
          # NON VÉRIFIÉE : elle n'a pas pu être construite lors de sa
          # rédaction, faute de Nix disponible. Le point le plus susceptible de
          # demander un ajustement est la construction du front, qui exige un
          # accès réseau que Nix refuse pendant la compilation — d'où
          # `npmDeps` ci-dessous, dont le hachage est à renseigner.
          app = pkgs.rustPlatform.buildRustPackage rec {
            pname = "coopacrypt";
            version = "0.2.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = guiNativeBuildInputs ++ [ pkgs.importNpmLock.npmConfigHook ];
            buildInputs = guiBuildInputs;

            npmDeps = pkgs.importNpmLock.buildNodeModules {
              npmRoot = ./app;
              inherit (pkgs) nodejs_22;
            };

            preBuild = ''
              # Le backend embarque le bundle du front : il doit exister avant
              # la compilation Rust.
              npm --prefix app run build
            '';

            cargoBuildFlags = [ "-p" "coopacrypt-app" ];
            doCheck = false;

            postInstall = ''
              install -Dm644 crates/coopacrypt-app/icons/128x128.png \
                $out/share/icons/hicolor/128x128/apps/coopacrypt.png

              mkdir -p $out/share/applications
              cat > $out/share/applications/coopacrypt.desktop <<EOF
              [Desktop Entry]
              Type=Application
              Name=CoopaCrypt
              Comment=Encrypted markdown notebook
              Exec=coopacrypt-app %f
              Icon=coopacrypt
              Terminal=false
              Categories=Utility;TextEditor;Security;
              EOF
            '';

            meta = with pkgs.lib; {
              inherit (self) description;
              homepage = "https://github.com/Coopaguard/CoopaCrypt";
              license = licenses.mit;
              mainProgram = "coopacrypt-app";
              platforms = platforms.linux;
            };
          };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = guiNativeBuildInputs ++ (with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
          ]);
          buildInputs = guiBuildInputs;
        };
      });
}
