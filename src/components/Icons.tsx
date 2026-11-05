const STROKE = { fill: "none", stroke: "currentColor", "stroke-width": 2, "stroke-linecap": "round", "stroke-linejoin": "round" } as const;

export const DiscIcon = () => (
  <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" {...STROKE}>
    <circle cx="12" cy="12" r="10" />
    <circle cx="12" cy="12" r="3" />
  </svg>
);

export const ImageIcon = () => (
  <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" {...STROKE}>
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <circle cx="9" cy="9" r="2" />
    <path d="M21 16l-5-5L5 21" />
  </svg>
);

export const FolderIcon = () => (
  <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" {...STROKE}>
    <path d="M3 7a2 2 0 0 1 2-2h4l2 3h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
  </svg>
);

export const FileIcon = () => (
  <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" {...STROKE}>
    <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
    <path d="M14 3v6h6" />
  </svg>
);
