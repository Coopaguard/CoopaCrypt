//! Binaire de l'application. Toute la logique vit dans la bibliothèque, ce qui
//! la rend testable sans démarrer d'interface graphique.

fn main() {
    coopacrypt_app_lib::run();
}
