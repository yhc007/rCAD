/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'cad-bg': '#1a1a1d',
        'cad-panel': '#242428',
        'cad-border': '#3a3a3f',
        'cad-accent': '#3b82f6',
        'cad-text': '#e4e4e7',
        'cad-text-muted': '#a1a1aa',
      },
    },
  },
  plugins: [],
};
