import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strict: true,
    host: host || false,
    hostName: host || undefined,
  },
  build: {
    target: "esnext",
    minify: "esbuild",
    sourcemap: false,
  },
  envPrefix: ["VITE_", "TAURI_"],
});
