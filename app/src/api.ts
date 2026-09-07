/**
 * Pont vers le backend Rust.
 *
 * La page ne chiffre rien et ne touche pas au système de fichiers : tout passe
 * par ces commandes. Le mot de passe n'est transmis qu'au déverrouillage et
 * n'est jamais conservé côté JavaScript (`UI.md` §5.1).
 */

import { invoke } from '@tauri-apps/api/core';

export interface SessionState {
  unlocked: boolean;
  path: string | null;
  remaining_secs: number | null;
}

export interface VaultInfo {
  version: number;
  memory_kib: number;
  iterations: number;
  parallelism: number;
  file_len: number;
  blocks: number;
  up_to_date: boolean;
}

export const api = {
  open: (path: string, password: string) => invoke<string>('vault_open', { path, password }),
  create: (path: string, password: string) => invoke<void>('vault_create', { path, password }),
  save: (content: string) => invoke<void>('vault_save', { content }),
  saveAs: (path: string, content: string, password: string) =>
    invoke<void>('vault_save_as', { path, content, password }),
  changePassword: (content: string, newPassword: string) =>
    invoke<void>('vault_change_password', { content, newPassword }),
  info: (path: string) => invoke<VaultInfo>('vault_info', { path }),
  lock: () => invoke<void>('vault_lock'),
  state: () => invoke<SessionState>('session_state'),
  touch: () => invoke<void>('session_touch'),
  /**
   * Coffre ouvert par double-clic ou « Ouvrir avec », s'il y en a un.
   *
   * La lecture **consomme** la valeur côté Rust : un rechargement de la page ne
   * rouvre pas le fichier du lancement précédent.
   */
  pendingVault: () => invoke<string | null>('pending_vault'),
};

/**
 * Émis quand un coffre attend d'être ouvert.
 *
 * L'événement ne porte pas le chemin — il invite seulement à appeler
 * `api.pendingVault()`, dont la lecture est atomique. Le faire voyager dans
 * l'événement risquerait une double ouverture.
 */
export const EVENT_PENDING = 'vault://pending';

/** Message d'erreur lisible, quelle que soit la forme rendue par le backend. */
export function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
