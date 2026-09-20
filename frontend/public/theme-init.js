// Applica subito il tema salvato (o la preferenza di sistema come primissima
// stima) prima che React idrati, per evitare un flash del tema sbagliato.
// ThemeProvider corregge poi con il calcolo alba/tramonto se la modalità è "auto".
// File esterno (non inline in layout.tsx) per restare compatibile con una CSP
// `script-src 'self'` (k8s/middleware-security-headers.yaml) senza dover
// gestire un nonce per-richiesta.
(function () {
  try {
    var stored = localStorage.getItem('notturni_theme_mode');
    var resolved = (stored === 'light' || stored === 'dark')
      ? stored
      : (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    document.documentElement.setAttribute('data-theme', resolved);
  } catch (e) {}
})();
