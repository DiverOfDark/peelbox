import { defineConfig } from '@tanstack/react-start/config'
import tsrPlugin from '@tanstack/router-plugin/vite'

export default defineConfig({
  vite: {
    plugins: [
      tsrPlugin({ autoCodeSplitting: true }),
    ],
  },
})
