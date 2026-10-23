import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri geliştirme sunucusu sabit port kullanır; CSP `connect-src` buna göre daraltılır.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_"],
  build: { target: "safari15", sourcemap: false },
});
