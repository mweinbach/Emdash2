import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';
import { installPlatformBridge } from './lib/platformBridge';
import { init as initGhostty } from 'ghostty-web';

async function bootstrap() {
  installPlatformBridge();
  try {
    await initGhostty();
  } catch (error) {
    console.error('[ghostty] failed to initialize', error);
  }

  const root = ReactDOM.createRoot(document.getElementById('root') as HTMLElement);

  // Avoid double-mount in dev which can duplicate PTY sessions
  root.render(<App />);
}

void bootstrap();
