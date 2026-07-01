import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const apiTarget = process.env.PYGCO_API_ORIGIN ?? "http://127.0.0.1:5174";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": new URL("./src", import.meta.url).pathname
    }
  },
  server: {
    port: 5173,
    proxy: {
      "/api": apiTarget
    }
  }
});
