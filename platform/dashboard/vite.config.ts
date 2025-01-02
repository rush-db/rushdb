import react from '@vitejs/plugin-react-swc'
import { resolve } from 'path'
import { defineConfig } from 'vite'
import checker from 'vite-plugin-checker'
import { createSvgIconsPlugin } from 'vite-plugin-svg-icons'

// https://vitejs.dev/config/
export default defineConfig({
  build: {
    sourcemap: true // for sentry
  },
  plugins: [
    react(),
    createSvgIconsPlugin({
      // Specify the icon folder to be cached
      iconDirs: [resolve(process.cwd(), 'src/assets/icons')],
      // Specify symbolId format
      symbolId: 'icon-[dir]-[name]',

      /**
       * custom insert position
       * @default: body-last
       */
      inject: 'body-last',

      /**
       * custom dom id
       * @default: __svg__icons__dom__
       */
      customDomId: '__svg__icons__dom__'
    }),
    checker({
      typescript: true
    })
  ],
  resolve: {
    // vite-tsconfig-paths
    alias: [{ find: '~', replacement: resolve(__dirname, 'src') }]
  },
  optimizeDeps: {
    exclude: ['@rushdb/javascript-sdk']
  },
  server: {
    port: 3005
  }
})
