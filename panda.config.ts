import { defineConfig } from "@pandacss/dev";

export default defineConfig({
  preflight: true,
  include: ["./src/**/*.{ts,tsx}"],
  exclude: [],
  outdir: "styled-system",

  theme: {
    tokens: {
      colors: {
        // Deep, rich dark palette — not generic gray
        bg: {
          deep: { value: "#0a0a0c" },
          base: { value: "#111114" },
          surface: { value: "#18181c" },
          elevated: { value: "#222228" },
          hover: { value: "#2a2a32" },
        },
        // Warm accent — amber/gold for a premium archival feel
        accent: {
          dim: { value: "#b8860b" },
          base: { value: "#d4a017" },
          bright: { value: "#f0c040" },
          subtle: { value: "rgba(212, 160, 23, 0.12)" },
        },
        // Text hierarchy
        text: {
          primary: { value: "#e8e6e3" },
          secondary: { value: "#9a9690" },
          muted: { value: "#5c5955" },
          inverse: { value: "#0a0a0c" },
        },
        // Semantic
        border: {
          subtle: { value: "rgba(255, 255, 255, 0.06)" },
          base: { value: "rgba(255, 255, 255, 0.10)" },
          strong: { value: "rgba(255, 255, 255, 0.16)" },
        },
        status: {
          success: { value: "#34d399" },
          error: { value: "#f87171" },
          warning: { value: "#fbbf24" },
          info: { value: "#60a5fa" },
        },
      },
      fonts: {
        heading: { value: "'Inter', -apple-system, system-ui, sans-serif" },
        body: { value: "'Inter', -apple-system, system-ui, sans-serif" },
        mono: { value: "'JetBrains Mono', 'SF Mono', monospace" },
        reading: {
          value: "'Literata', 'Georgia', 'Times New Roman', serif",
        },
      },
      fontSizes: {
        xs: { value: "0.7rem" },
        sm: { value: "0.8rem" },
        md: { value: "0.9rem" },
        lg: { value: "1.05rem" },
        xl: { value: "1.25rem" },
        "2xl": { value: "1.6rem" },
        "3xl": { value: "2rem" },
      },
      radii: {
        sm: { value: "4px" },
        md: { value: "8px" },
        lg: { value: "12px" },
        xl: { value: "16px" },
        full: { value: "9999px" },
      },
      spacing: {
        xs: { value: "4px" },
        sm: { value: "8px" },
        md: { value: "16px" },
        lg: { value: "24px" },
        xl: { value: "32px" },
        "2xl": { value: "48px" },
        "3xl": { value: "64px" },
      },
      easings: {
        smooth: { value: "cubic-bezier(0.25, 0.1, 0.25, 1)" },
        snappy: { value: "cubic-bezier(0.2, 0, 0, 1)" },
        bounce: { value: "cubic-bezier(0.34, 1.56, 0.64, 1)" },
      },
      durations: {
        fast: { value: "120ms" },
        normal: { value: "200ms" },
        slow: { value: "350ms" },
      },
    },
  },

  globalCss: {
    "*, *::before, *::after": {
      boxSizing: "border-box",
      margin: 0,
      padding: 0,
    },
    html: {
      fontSize: "15px",
      WebkitFontSmoothing: "antialiased",
      MozOsxFontSmoothing: "grayscale",
    },
    body: {
      fontFamily: "body",
      bg: "bg.deep",
      color: "text.primary",
      lineHeight: 1.6,
      overflow: "hidden",
      userSelect: "none",
    },
    // Allow text selection in reading areas
    ".selectable": {
      userSelect: "text",
    },
    // Scrollbar styling
    "::-webkit-scrollbar": {
      width: "6px",
    },
    "::-webkit-scrollbar-track": {
      background: "transparent",
    },
    "::-webkit-scrollbar-thumb": {
      background: "rgba(255, 255, 255, 0.08)",
      borderRadius: "3px",
    },
    "::-webkit-scrollbar-thumb:hover": {
      background: "rgba(255, 255, 255, 0.14)",
    },
  },
});
