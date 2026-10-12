<script lang="ts" module>
  // Контуры иконок с холста дизайна, сетка 24×24, обводка без заливки.
  const PATHS = {
    key: '<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9"/><path d="M16 7l3 3"/><path d="M14 9l2 2"/>',
    file: '<path d="M14 3H7a2 2 0 00-2 2v14a2 2 0 002 2h10a2 2 0 002-2V8z"/><path d="M14 3v5h5"/>',
    eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    "eye-off":
      '<path d="M3 3l18 18"/><path d="M10.6 6.1A9.8 9.8 0 0112 6c6 0 10 6 10 6a17 17 0 01-3.2 3.7M6.6 7.6A17 17 0 002 12s4 7 10 7a9.7 9.7 0 004.2-1"/><path d="M9.9 9.9a3 3 0 004.2 4.2"/>',
    lock: '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 018 0v4"/>',
    unlock: '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 017.5-1.5"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    import: '<path d="M12 4v11M7 10l5 5 5-5"/><path d="M5 20h14"/>',
    back: '<path d="M19 12H5M11 6l-6 6 6 6"/>',
    check: '<path d="M5 12l5 5 9-10"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>',
    close: '<path d="M6 6l12 12M18 6L6 18"/>',
    generate:
      '<path d="M20 11a8 8 0 00-14.5-4.5L4 8"/><path d="M4 4v4h4"/><path d="M4 13a8 8 0 0014.5 4.5L20 16"/><path d="M20 20v-4h-4"/>',
    warning: '<path d="M12 3l9 16H3z"/><path d="M12 10v4M12 17h.01"/>',
    error: '<circle cx="12" cy="12" r="9"/><path d="M12 8v5M12 16h.01"/>',
    settings:
      '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
    copy: '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V6a2 2 0 012-2h9"/>',
    chevron: '<path d="M9 6l6 6-6 6"/>',
    edit: '<path d="M4 20h4L19 9l-4-4L4 16z"/>',
    trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
    "lock-memory": '<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11h8"/><path d="M8 11V7a4 4 0 017.5-2"/>',
    document: '<path d="M14 3H7a2 2 0 00-2 2v14a2 2 0 002 2h10a2 2 0 002-2V8z"/><path d="M14 3v5h5"/><path d="M9 13h6M9 17h4"/>',
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  interface Props {
    name: IconName;
    size?: number;
    stroke?: number;
    color?: string;
  }

  let { name, size = 16, stroke = 1.75, color = "currentColor" }: Props = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke={color}
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <!-- Разметка берётся только из PATHS выше, пользовательских данных в ней нет -->
  {@html PATHS[name]}
</svg>

<style>
  svg {
    flex: none;
  }
</style>
