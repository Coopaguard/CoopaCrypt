/**
 * Application CoopaCrypt.
 *
 * Modèle central (`UI.md` §1) : le document est **un seul markdown continu**.
 * Le découpage en chapitres est une vue recalculée à chaque frappe ; l'éditeur
 * n'affiche qu'une tranche, mais toute modification s'applique au document
 * complet, et la recherche porte toujours sur son intégralité.
 */

import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import type { EditorView } from '@codemirror/view';

import { api, errorMessage, EVENT_PENDING } from './api';
import { createEditor, restoreScroll, revealRange, scrollTop, setSlice } from './editor';
import { findAll, groupByChapter, nextMatch, replaceAll, type Match } from './search';
import { chapterAt, replaceChapter, splitChapters, type Chapter } from './structure';
import './styles.css';

/** Cadence d'interrogation de l'état de session, pour le verrouillage automatique. */
const SESSION_POLL_MS = 10_000;
/** Un signal d'activité au plus toutes les 20 s : inutile de saturer l'IPC. */
const TOUCH_THROTTLE_MS = 20_000;

interface State {
  /** Document markdown complet, déchiffré. */
  document: string;
  chapters: Chapter[];
  /** Index du chapitre affiché. */
  current: number;
  dirty: boolean;
  matches: Match[];
  /** Position de défilement mémorisée par chapitre, le temps de la session. */
  scrollByChapter: Map<number, number>;
}

const state: State = {
  document: '',
  chapters: splitChapters(''),
  current: 0,
  dirty: false,
  matches: [],
  scrollByChapter: new Map(),
};

let view: EditorView;
let lastTouch = 0;
/** Vrai pendant une opération de chiffrement en cours. */
let busy = false;
/**
 * Vrai pendant un enchaînement complet (fenêtre de fichier, saisie, chiffrement).
 *
 * Distinct de `busy`, qui ne couvre que le calcul. Sans ce garde, deux clics
 * rapprochés lancent deux enchaînements et donc **deux fenêtres natives
 * empilées** — ce qui donne l'impression que le sélecteur de fichier réapparaît
 * sans raison, et perturbe le curseur.
 */
let flowActive = false;
/** Vrai pendant une mise à jour programmée de l'éditeur, pour ignorer l'écho. */
let applyingSlice = false;

// --- éléments du document -------------------------------------------------

const el = <T extends HTMLElement>(id: string) => {
  const found = document.getElementById(id);
  if (!found) throw new Error(`élément « ${id} » absent du document`);
  return found as T;
};

const screenLocked = el('screen-locked');
const screenVault = el('screen-vault');
const outlineList = el('outline-list');
const searchResults = el('search-results');
const statusText = el('status-text');
const statusDirty = el('status-dirty');
const statusTimer = el('status-timer');
const unlockError = el('unlock-error');
const busyOverlay = el('busy-overlay');
const busyLabel = el('busy-label');
const searchPanel = el('search-panel');
const searchInput = el<HTMLInputElement>('search-input');
const replaceInput = el<HTMLInputElement>('replace-input');
const caseSensitive = el<HTMLInputElement>('search-case');

// --- rendu ----------------------------------------------------------------

function refreshOutline() {
  outlineList.replaceChildren(
    ...state.chapters.map((chapter) => {
      const item = document.createElement('li');

      const button = document.createElement('button');
      button.className = 'chapter' + (chapter.index === state.current ? ' active' : '');
      button.textContent = chapter.title;
      button.title = chapter.title;
      button.addEventListener('click', () => showChapter(chapter.index));
      item.append(button);

      if (chapter.subsections.length > 0 && chapter.index === state.current) {
        const sublist = document.createElement('ul');
        sublist.className = 'subsections';
        for (const sub of chapter.subsections) {
          const subItem = document.createElement('li');
          const subButton = document.createElement('button');
          subButton.className = `subsection level-${Math.min(sub.level, 4)}`;
          subButton.textContent = sub.title;
          subButton.title = sub.title;
          // Une sous-partie ne change pas de chapitre : elle fait défiler
          // à l'intérieur de celui-ci (`UI.md` §2.1).
          subButton.addEventListener('click', () => {
            const local = sub.from - chapter.from;
            revealRange(view, local, local);
          });
          subItem.append(subButton);
          sublist.append(subItem);
        }
        item.append(sublist);
      }

      return item;
    }),
  );
}

function refreshStatus() {
  statusDirty.textContent = state.dirty ? '● non enregistré' : '';
  statusDirty.className = state.dirty ? 'dirty' : '';
}

/**
 * Affiche un message là où l'utilisateur le verra.
 *
 * La barre d'état vit dans l'écran du coffre : verrouillé, elle est masquée. Un
 * message émis depuis l'écran d'accueil doit donc s'y afficher aussi, faute de
 * quoi l'échec est parfaitement silencieux — c'est ce qui rendait un échec de
 * création de coffre invisible.
 */
function setStatus(message: string, kind: 'info' | 'error' = 'info') {
  statusText.textContent = message;
  statusText.className = kind;
  if (screenVault.hidden && kind === 'error') unlockError.textContent = message;
}

function reportError(error: unknown) {
  setStatus(errorMessage(error), 'error');
}

/**
 * Signale une opération en cours.
 *
 * La dérivation Argon2id prend quelques centaines de millisecondes, désormais
 * hors du thread de l'interface. La fenêtre reste donc réactive — raison de plus
 * pour montrer explicitement que quelque chose travaille, sinon un clic sans
 * effet visible passe pour un bug.
 *
 * Le voile bloque aussi les interactions : lancer un second enregistrement
 * pendant le premier n'aurait aucun sens.
 */
function setBusy(active: boolean, label = '') {
  busy = active;
  busyLabel.textContent = label;
  busyOverlay.hidden = !active;
  document.body.classList.toggle('is-busy', active);
}

/**
 * Enveloppe une opération longue : voile, curseur d'attente, message d'état.
 *
 * Le garde contre la réentrance est ici plutôt que dans chaque appelant : c'est
 * le seul endroit qui connaît l'état réel.
 */
/**
 * Sérialise un enchaînement utilisateur du début à la fin.
 *
 * Un seul enchaînement à la fois : tant qu'une fenêtre de sélection ou de saisie
 * est ouverte, un nouveau clic est ignoré plutôt que d'ouvrir une seconde
 * fenêtre par-dessus la première.
 */
async function withFlow(work: () => Promise<void>): Promise<void> {
  if (flowActive) return;
  flowActive = true;
  try {
    await work();
  } finally {
    flowActive = false;
  }
}

async function withBusy<T>(label: string, work: () => Promise<T>): Promise<T | undefined> {
  if (busy) return undefined;
  setBusy(true, label);
  setStatus(label);
  try {
    return await work();
  } catch (error) {
    reportError(error);
    return undefined;
  } finally {
    setBusy(false);
  }
}

// --- navigation entre chapitres -------------------------------------------

function showChapter(index: number, caretLocal = 0) {
  if (index < 0 || index >= state.chapters.length) return;

  if (state.chapters[state.current]) {
    state.scrollByChapter.set(state.current, scrollTop(view));
  }

  state.current = index;
  const chapter = state.chapters[index];

  applyingSlice = true;
  setSlice(view, state.document.slice(chapter.from, chapter.to), {
    anchor: caretLocal,
    head: caretLocal,
  });
  applyingSlice = false;

  restoreScroll(view, state.scrollByChapter.get(index) ?? 0);
  refreshOutline();
}

/**
 * Prend en compte une modification de la tranche affichée.
 *
 * Le point délicat : ajouter ou retirer un `#` scinde ou fusionne des
 * chapitres (`UI.md` §2.3). Il faut alors resynchroniser l'éditeur **sans**
 * perdre la position du curseur, et sans re-découper à chaque frappe ordinaire.
 */
function onSliceChanged(sliceText: string) {
  if (applyingSlice) return;

  const previous = state.chapters[state.current];
  const caretLocal = view.state.selection.main.head;
  const caretAbsolute = previous.from + caretLocal;

  state.document = replaceChapter(state.document, previous, sliceText);
  state.chapters = splitChapters(state.document);
  state.dirty = true;

  const target = chapterAt(state.chapters, Math.min(caretAbsolute, state.document.length));
  const expected = state.document.slice(target.from, target.to);

  if (target.index !== state.current || expected !== sliceText) {
    // La structure a bougé : re-trancher en replaçant le curseur là où il était.
    showChapter(target.index, Math.max(0, caretAbsolute - target.from));
  } else {
    refreshOutline();
  }

  refreshStatus();
  if (state.matches.length > 0) runSearch();
}

// --- recherche ------------------------------------------------------------

function runSearch() {
  const needle = searchInput.value;
  state.matches = findAll(state.document, needle, state.chapters, caseSensitive.checked);

  const groups = groupByChapter(state.matches, state.chapters);
  searchResults.replaceChildren();

  if (needle.length === 0) return;

  if (state.matches.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'empty';
    empty.textContent = 'Aucun résultat.';
    searchResults.append(empty);
    return;
  }

  const total = document.createElement('p');
  total.className = 'result-count';
  total.textContent =
    `${state.matches.length} occurrence${state.matches.length > 1 ? 's' : ''} ` +
    `dans ${groups.length} chapitre${groups.length > 1 ? 's' : ''}`;
  searchResults.append(total);

  for (const group of groups) {
    const heading = document.createElement('p');
    heading.className = 'result-chapter';
    heading.textContent = `${group.chapterTitle} (${group.matches.length})`;
    searchResults.append(heading);

    for (const match of group.matches) {
      const button = document.createElement('button');
      button.className = 'result';
      button.textContent = match.excerpt;
      button.addEventListener('click', () => goToMatch(match));
      searchResults.append(button);
    }
  }
}

/** Ouvre le chapitre du résultat si nécessaire, puis y sélectionne l'occurrence. */
function goToMatch(match: Match) {
  const chapter = state.chapters[match.chapterIndex];
  if (match.chapterIndex !== state.current) {
    showChapter(match.chapterIndex);
    setStatus(`Résultat dans « ${chapter.title} »`);
  }
  revealRange(view, match.from - chapter.from, match.to - chapter.from);
}

function goToNextMatch() {
  if (state.matches.length === 0) {
    runSearch();
    if (state.matches.length === 0) return;
  }
  const chapter = state.chapters[state.current];
  const caretAbsolute = chapter.from + view.state.selection.main.head;
  const match = nextMatch(state.matches, caretAbsolute);
  if (match) goToMatch(match);
}

function doReplaceAll() {
  runSearch();
  if (state.matches.length === 0) {
    setStatus('Rien à remplacer.');
    return;
  }

  const result = replaceAll(state.document, state.matches, replaceInput.value);
  state.document = result.text;
  state.chapters = splitChapters(state.document);
  state.dirty = true;

  showChapter(Math.min(state.current, state.chapters.length - 1));
  runSearch();
  refreshStatus();

  // Annoncer les chapitres touchés, et pas seulement le nombre d'occurrences :
  // du contenu invisible a pu être modifié (`UI.md` §4.3).
  setStatus(
    `${result.count} remplacement${result.count > 1 ? 's' : ''} ` +
      `dans ${result.chaptersTouched} chapitre${result.chaptersTouched > 1 ? 's' : ''}.`,
  );
}

function toggleSearch(show: boolean) {
  searchPanel.hidden = !show;
  if (show) {
    const selection = view.state.sliceDoc(
      view.state.selection.main.from,
      view.state.selection.main.to,
    );
    if (selection && !selection.includes('\n')) searchInput.value = selection;
    searchInput.focus();
    searchInput.select();
    runSearch();
  } else {
    state.matches = [];
    searchResults.replaceChildren();
    view.focus();
  }
}

// --- cycle de vie du coffre -----------------------------------------------

function loadDocument(content: string, path: string) {
  state.document = content;
  state.chapters = splitChapters(content);
  state.current = 0;
  state.dirty = false;
  state.matches = [];
  state.scrollByChapter.clear();

  screenLocked.hidden = true;
  screenVault.hidden = false;

  showChapter(0);
  refreshStatus();
  searchResults.replaceChildren();
  setStatus(fileName(path));
}

/**
 * Verrouille : efface le document, le volet et la prévisualisation.
 *
 * La seule arborescence des chapitres est déjà une fuite d'information
 * (`UI.md` §5.1) : le volet doit disparaître au même titre que l'éditeur.
 */
async function lock(reason?: string) {
  await api.lock().catch(() => undefined);

  state.document = '';
  state.chapters = splitChapters('');
  state.current = 0;
  state.dirty = false;
  state.matches = [];
  state.scrollByChapter.clear();

  applyingSlice = true;
  setSlice(view, '');
  applyingSlice = false;

  outlineList.replaceChildren();
  searchResults.replaceChildren();
  searchPanel.hidden = true;

  screenVault.hidden = true;
  screenLocked.hidden = false;
  statusTimer.textContent = '';
  unlockError.textContent = reason ?? '';
}

function openVault() {
  return withFlow(async () => {
    if (!(await confirmDiscard())) return;
    unlockError.textContent = '';

    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'Coffre CoopaCrypt', extensions: ['coocrypt'] }],
    });
    if (typeof selected !== 'string') return;

    await unlockFile(selected);
  });
}

/**
 * Ouvre le coffre transmis au lancement, par double-clic ou « Ouvrir avec ».
 *
 * Appelée au démarrage puis à chaque `EVENT_PENDING`, c'est-à-dire lorsqu'un
 * second lancement a été absorbé par l'instance déjà en place.
 *
 * Le chemin n'est réclamé qu'à l'intérieur de `withFlow` : si un enchaînement
 * est déjà en cours, il reste en attente côté Rust au lieu d'être consommé et
 * perdu.
 */
function openPendingVault() {
  return withFlow(async () => {
    const path = await api.pendingVault();
    if (!path) return;
    if (!(await confirmDiscard())) return;

    unlockError.textContent = '';
    await unlockFile(path);
  });
}

/**
 * Déverrouille un coffre, en redemandant le mot de passe autant de fois que
 * nécessaire.
 *
 * La boucle porte sur le **mot de passe**, jamais sur le choix du fichier :
 * refaire désigner le même fichier après une faute de frappe n'aurait aucun
 * sens. L'utilisateur sort en annulant la saisie.
 */
async function unlockFile(path: string) {
  let message = '';
  for (;;) {
    const password = await askPassword(fileName(path), message);
    if (password === null) {
      setStatus('Ouverture annulée.');
      return;
    }
    setBusy(true, 'Déchiffrement…');
    setStatus('Déchiffrement…');
    try {
      const content = await api.open(path, password);
      setBusy(false);
      loadDocument(content, path);
      return;
    } catch (error) {
      setBusy(false);
      // Le message est réaffiché dans la fenêtre de saisie elle-même, au plus
      // près de l'endroit où l'utilisateur doit corriger.
      message = errorMessage(error);
      setStatus(message, 'error');
    }
  }
}

function newVault() {
  return withFlow(async () => {
    if (!(await confirmDiscard())) return;
    unlockError.textContent = '';

    const selected = await saveDialog({
      filters: [{ name: 'Coffre CoopaCrypt', extensions: ['coocrypt'] }],
      defaultPath: 'coffre.coocrypt',
    });
    if (typeof selected !== 'string') return;

    const password = await askNewPassword();
    if (password === null) return;

    const created = await withBusy('Création du coffre…', async () => {
      await api.create(selected, password);
      return true;
    });

    if (created) {
      loadDocument('', selected);
      setStatus(`Coffre créé : ${fileName(selected)}`);
    }
  });
}

async function save() {
  if (screenVault.hidden) return;
  const done = await withBusy('Enregistrement…', async () => {
    await api.save(state.document);
    return true;
  });
  if (done) {
    state.dirty = false;
    refreshStatus();
    setStatus('Enregistré.');
  }
}

function saveAs() {
  return withFlow(saveAsFlow);
}

async function saveAsFlow() {
  const selected = await saveDialog({
    filters: [{ name: 'Coffre CoopaCrypt', extensions: ['coocrypt'] }],
    defaultPath: 'coffre.coocrypt',
  });
  if (typeof selected !== 'string') return;

  const password = await askNewPassword();
  if (password === null) return;

  const done = await withBusy('Enregistrement…', async () => {
    await api.saveAs(selected, state.document, password);
    return true;
  });
  if (done) {
    state.dirty = false;
    refreshStatus();
    setStatus(`Enregistré sous ${fileName(selected)}`);
  }
}

function changePassword() {
  return withFlow(changePasswordFlow);
}

async function changePasswordFlow() {
  const password = await askNewPassword();
  if (password === null) return;
  const done = await withBusy('Changement du mot de passe…', async () => {
    await api.changePassword(state.document, password);
    return true;
  });
  if (done) {
    state.dirty = false;
    refreshStatus();
    setStatus('Mot de passe changé.');
  }
}

// --- saisies --------------------------------------------------------------

/**
 * Demande un mot de passe existant.
 *
 * Un champ `password` dans un dialogue de la page plutôt qu'un `prompt()` natif :
 * la saisie ne doit pas s'afficher en clair.
 *
 * Le garde `dialog.open` est indispensable. `close()` ne déclenche l'événement
 * `close` qu'au tour de boucle suivant ; si le dialogue a été rouvert entre
 * temps, cet événement en retard ne doit surtout pas annuler la nouvelle
 * demande.
 */
function askPassword(label: string, initialError = ''): Promise<string | null> {
  return new Promise((resolve) => {
    const dialog = el<HTMLDialogElement>('password-dialog');
    const input = el<HTMLInputElement>('password-input');
    const error = el('password-error');

    el('password-title').textContent = label;
    input.value = '';
    error.textContent = initialError;

    let settled = false;

    const finish = (value: string | null) => {
      if (settled) return;
      settled = true;
      dialog.removeEventListener('close', onClose);
      // Ne pas laisser le secret traîner dans le DOM.
      input.value = '';
      if (dialog.open) dialog.close();
      resolve(value);
    };

    function onClose() {
      if (!dialog.open) finish(null);
    }

    const submit = () => {
      if (input.value.length === 0) {
        error.textContent = 'Le mot de passe ne peut pas être vide.';
        return;
      }
      finish(input.value);
    };

    dialog.addEventListener('close', onClose);
    el('password-ok').onclick = submit;
    el('password-cancel').onclick = () => finish(null);
    input.onkeydown = (event) => {
      if (event.key === 'Enter') submit();
      if (event.key === 'Escape') finish(null);
    };

    dialog.showModal();
    input.focus();
  });
}

/**
 * Demande un nouveau mot de passe et sa confirmation, dans **une seule**
 * fenêtre.
 *
 * Enchaîner deux fois `askPassword` sur le même élément `<dialog>` ne
 * fonctionne pas : l'événement `close` différé du premier tour arrive après la
 * réouverture et annule le second. Une fenêtre à deux champs supprime le
 * problème et permet de signaler la divergence sur place.
 *
 * Une faute de frappe non détectée rend le coffre définitivement inaccessible :
 * ni récupération, ni indice, ni question secrète.
 */
function askNewPassword(): Promise<string | null> {
  return new Promise((resolve) => {
    const dialog = el<HTMLDialogElement>('newpassword-dialog');
    const first = el<HTMLInputElement>('newpassword-input');
    const second = el<HTMLInputElement>('newpassword-confirm');
    const error = el('newpassword-error');

    first.value = '';
    second.value = '';
    error.textContent = '';

    let settled = false;

    const finish = (value: string | null) => {
      if (settled) return;
      settled = true;
      dialog.removeEventListener('close', onClose);
      first.value = '';
      second.value = '';
      if (dialog.open) dialog.close();
      resolve(value);
    };

    function onClose() {
      if (!dialog.open) finish(null);
    }

    const submit = () => {
      if (first.value.length === 0) {
        error.textContent = 'Le mot de passe ne peut pas être vide.';
        first.focus();
        return;
      }
      if (first.value !== second.value) {
        error.textContent = 'Les deux saisies diffèrent.';
        second.focus();
        second.select();
        return;
      }
      // Avertissement, pas blocage : le choix appartient à l'utilisateur.
      if (first.value.length < 12 || first.value.trim().split(/\s+/).length < 3) {
        setStatus(
          'Phrase de passe courte : le chiffrement ne compense pas un mot de passe devinable.',
          'error',
        );
      }
      finish(first.value);
    };

    dialog.addEventListener('close', onClose);
    el('newpassword-ok').onclick = submit;
    el('newpassword-cancel').onclick = () => finish(null);
    for (const field of [first, second]) {
      field.onkeydown = (event) => {
        if (event.key === 'Enter') submit();
        if (event.key === 'Escape') finish(null);
      };
    }

    dialog.showModal();
    first.focus();
  });
}

/** Réponse de l'utilisateur devant des modifications non enregistrées. */
type QuitChoice = 'save' | 'discard' | 'cancel';

/**
 * Demande quoi faire des modifications en cours.
 *
 * Trois issues, dont **enregistrer** : ne proposer que « abandonner ou annuler »
 * forcerait à sortir du dialogue pour faire la chose la plus évidente.
 */
function askUnsaved(): Promise<QuitChoice> {
  return new Promise((resolve) => {
    const dialog = el<HTMLDialogElement>('quit-dialog');
    let settled = false;

    const finish = (choice: QuitChoice) => {
      if (settled) return;
      settled = true;
      dialog.removeEventListener('close', onClose);
      if (dialog.open) dialog.close();
      resolve(choice);
    };

    function onClose() {
      if (!dialog.open) finish('cancel');
    }

    dialog.addEventListener('close', onClose);
    el('quit-save').onclick = () => finish('save');
    el('quit-discard').onclick = () => finish('discard');
    el('quit-cancel').onclick = () => finish('cancel');

    dialog.showModal();
  });
}

/**
 * Traite les modifications en attente avant une action destructive.
 *
 * Rend `false` si l'action doit être abandonnée — soit que l'utilisateur ait
 * annulé, soit que l'enregistrement ait échoué. Dans ce dernier cas, poursuivre
 * détruirait le travail que l'on vient justement de ne pas réussir à sauver.
 */
async function resolveUnsaved(): Promise<boolean> {
  if (!state.dirty) return true;

  const choice = await askUnsaved();
  if (choice === 'cancel') return false;
  if (choice === 'discard') return true;

  await save();
  return !state.dirty;
}

async function confirmDiscard(): Promise<boolean> {
  return resolveUnsaved();
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

// --- session --------------------------------------------------------------

function noteActivity() {
  const now = Date.now();
  if (now - lastTouch < TOUCH_THROTTLE_MS) return;
  lastTouch = now;
  void api.touch().catch(() => undefined);
}

/**
 * Interroge périodiquement l'état de session.
 *
 * L'échéance fait autorité **côté Rust** : un minuteur dans la page ne serait
 * qu'un confort, rien ne garantissant son exécution si la vue est suspendue.
 */
async function pollSession() {
  try {
    const session = await api.state();
    if (!session.unlocked && !screenVault.hidden) {
      await lock('Session verrouillée après inactivité.');
      return;
    }
    if (session.unlocked && session.remaining_secs !== null) {
      const minutes = Math.ceil(session.remaining_secs / 60);
      statusTimer.textContent = `verrouillage dans ~${minutes} min`;
    }
  } catch {
    // Le backend est injoignable : ne pas laisser un coffre ouvert dans le vide.
    if (!screenVault.hidden) await lock('Connexion au backend perdue.');
  }
}

// --- démarrage ------------------------------------------------------------

function wireUp() {
  view = createEditor(el('editor'), {
    onChange: onSliceChanged,
    onActivity: noteActivity,
    onSave: () => void save(),
    onFind: () => toggleSearch(true),
    onFindNext: goToNextMatch,
  });

  el('btn-new').addEventListener('click', () => void newVault());
  el('btn-open').addEventListener('click', () => void openVault());
  el('btn-unlock-open').addEventListener('click', () => void openVault());
  el('btn-unlock-new').addEventListener('click', () => void newVault());
  el('btn-save').addEventListener('click', () => void save());
  el('btn-save-as').addEventListener('click', () => void saveAs());
  el('btn-chpass').addEventListener('click', () => void changePassword());
  el('btn-lock').addEventListener('click', () => void lock());
  el('btn-find').addEventListener('click', () => toggleSearch(true));
  el('btn-search-close').addEventListener('click', () => toggleSearch(false));
  el('btn-search-next').addEventListener('click', goToNextMatch);
  el('btn-replace-all').addEventListener('click', doReplaceAll);

  searchInput.addEventListener('input', runSearch);
  caseSensitive.addEventListener('change', runSearch);
  searchInput.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') goToNextMatch();
    if (event.key === 'Escape') toggleSearch(false);
  });

  document.addEventListener('keydown', (event) => {
    const mod = event.ctrlKey || event.metaKey;
    if (mod && event.key === 's') {
      event.preventDefault();
      void save();
    }
    if (mod && event.key === 'f') {
      event.preventDefault();
      toggleSearch(true);
    }
    if (mod && event.key === 'l') {
      event.preventDefault();
      void lock();
    }
    if (event.key === 'F3') {
      event.preventDefault();
      goToNextMatch();
    }
  });

  for (const type of ['pointerdown', 'keydown'] as const) {
    document.addEventListener(type, noteActivity, { passive: true });
  }

  // `beforeunload` ne se déclenche pas ici : sous Tauri la fenêtre est fermée
  // par le système, sans déchargement de page. Il faut intercepter la demande
  // de fermeture côté Tauri.
  void getCurrentWindow().onCloseRequested(async (event) => {
    if (!state.dirty) return;
    event.preventDefault();
    if (await resolveUnsaved()) {
      await getCurrentWindow().destroy();
    }
  });

  window.setInterval(() => void pollSession(), SESSION_POLL_MS);
  void pollSession();

  // Un double-clic sur un coffre alors que l'application tourne déjà est
  // absorbé par l'instance en place, qui prévient par cet événement.
  void listen(EVENT_PENDING, () => void openPendingVault());

  // Lancement par double-clic : le chemin attend déjà côté Rust. Sans coffre à
  // ouvrir, l'appel ne fait rien et l'écran verrouillé reste affiché.
  void openPendingVault();
}

wireUp();
