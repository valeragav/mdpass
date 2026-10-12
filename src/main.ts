import { mount } from "svelte";

// Шрифты лежат в сборке: менеджер паролей не ходит в сеть.
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./styles/theme.css";

import App from "./App.svelte";

mount(App, { target: document.getElementById("app")! });
