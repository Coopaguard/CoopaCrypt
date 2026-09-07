/**
 * Rendu de la prévisualisation markdown.
 *
 * # Pourquoi la désinfection n'est pas optionnelle
 *
 * Le contenu du coffre est rendu en HTML **dans la vue Tauri**, laquelle a
 * accès au pont IPC. Un `<script>` ou un `onerror=` glissé dans le document
 * s'exécuterait donc avec les droits de l'application et pourrait appeler
 * `vault_save`, lire le document, ou l'exfiltrer.
 *
 * Le contenu vient de l'utilisateur lui-même, mais un coffre se copie, se
 * partage et se synchronise : il n'est pas nécessairement resté sous son seul
 * contrôle. Le rendu est donc systématiquement désinfecté, et la politique de
 * sécurité de contenu déclarée dans `tauri.conf.json` interdit en second rideau
 * tout script en ligne.
 */

import DOMPurify from 'dompurify';
import { marked } from 'marked';

marked.setOptions({
  // Un retour à la ligne simple devient un `<br>` : c'est ce qu'attend
  // quelqu'un qui prend des notes, plutôt que la fusion des lignes du markdown
  // strict.
  breaks: true,
  gfm: true,
});

/** Rend un fragment markdown en HTML désinfecté. */
export function renderMarkdown(source: string): string {
  const raw = marked.parse(source, { async: false });
  return DOMPurify.sanitize(raw, {
    // Aucune balise ni attribut permettant l'exécution de code.
    FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form', 'input'],
    FORBID_ATTR: ['style', 'srcset', 'formaction'],
  });
}

/**
 * Refuse d'ouvrir un lien vers l'extérieur depuis la prévisualisation.
 *
 * Une requête réseau partant de la vue trahirait le contenu du coffre — ne
 * serait-ce que par l'URL appelée. Les liens restent visibles et copiables,
 * mais inertes.
 */
export function neutralizeLinks(container: HTMLElement): void {
  container.addEventListener('click', (event) => {
    const target = (event.target as HTMLElement | null)?.closest('a');
    if (target) event.preventDefault();
  });
}
