import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const css = readFileSync(fileURLToPath(new URL('./styles.css', import.meta.url)), 'utf8');

/**
 * Ces règles semblent anodines mais leur absence casse l'application entière.
 *
 * `[hidden]` n'est défini que par la feuille de style du navigateur, qu'un
 * style auteur écrase toujours — indépendamment de la spécificité. Comme
 * `.locked` et `.vault` déclarent chacun un `display`, sans règle explicite les
 * deux écrans restent affichés en permanence, empilés à 100 % de hauteur. Seul
 * le premier est alors visible, et ouvrir un coffre semble ne rien faire.
 */
describe('feuille de style', () => {
  it('neutralise l’attribut hidden avec !important', () => {
    const rule = css.match(/\[hidden\]\s*\{[^}]*\}/);
    expect(rule, 'aucune règle [hidden] dans styles.css').not.toBeNull();
    expect(rule?.[0]).toMatch(/display:\s*none\s*!important/);
  });

  it('déclare bien un display sur les deux écrans, ce qui rend la règle nécessaire', () => {
    // Si cette attente tombait, la règle ci-dessus resterait néanmoins correcte ;
    // le test documente pourquoi elle existe.
    expect(css).toMatch(/\.locked\s*\{[^}]*display:/);
    expect(css).toMatch(/\.vault\s*\{[^}]*display:/);
  });
});
