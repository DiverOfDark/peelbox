import { defineConfig } from '@tanstack/start/config'
import tsrPlugin from '@tanstack/router-plugin/vite'

export default defineConfig({
  vite: {
    plugins: [
      tsrPlugin({ autoCodeSplitting: true }),
    ],
  },
})
