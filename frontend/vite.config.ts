import path from "path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/notetaker.v1.AuthService": {
        target: "http://127.0.0.1:50051",
        changeOrigin: true,
      },
      "/notetaker.v1.CalendarService": {
        target: "http://127.0.0.1:50051",
        changeOrigin: true,
      },
    },
  },
})
