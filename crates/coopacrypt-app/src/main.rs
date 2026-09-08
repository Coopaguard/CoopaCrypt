//! Binaire de l'application. Toute la logique vit dans la bibliothèque, ce qui
//! la rend testable sans démarrer d'interface graphique.

// Sans cet attribut, Windows ouvre une console derrière la fenêtre.
//
// Il doit être posé **ici** et non dans la bibliothèque : `windows_subsystem`
// ne vaut que pour la racine du crate effectivement lié en exécutable. Placé
// dans `lib.rs`, il est silencieusement sans effet — l'application se lançait
// avec un terminal.
//
// Conservé en débogage : la sortie d'erreur y sert au diagnostic.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    coopacrypt_app_lib::run();
}
