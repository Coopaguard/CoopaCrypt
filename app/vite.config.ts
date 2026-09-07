import { defineConfig } from 'vite';

// Tauri sert le front depuis ce port en développement et depuis `dist` en
// production. `clearScreen: false` laisse visibles les erreurs de compilation
// Rust, qui défilent dans le même terminal.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: 'es2022',
    // Aucune carte de source en production : elle exposerait le code de
    // l'application sans bénéfice pour l'utilisateur.
    sourcemap: false,
    emptyOutDir: true,
  },
});
