import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
// Self-hosted brand fonts — bundled by Vite, no network dependency.
// Geist Sans + Geist Mono: the SF Pro-superfamily analogue (OFL, redistributable).
import '@fontsource/geist-sans/400.css';
import '@fontsource/geist-sans/500.css';
import '@fontsource/geist-sans/600.css';
import '@fontsource/geist-sans/700.css';
import '@fontsource/geist-mono/400.css';
import '@fontsource/geist-mono/500.css';
import '@fontsource/geist-mono/600.css';
import '@fontsource/geist-mono/700.css';
import App from './App';
import { startSplash } from './lib/splash';
import './index.css';

// Start the boot bar the moment this bundle runs — earlier than App's first
// effect, which costs a React mount. Everything after this point is what the
// bar is actually waiting on.
startSplash();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
