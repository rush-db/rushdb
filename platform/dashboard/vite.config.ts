import { sentryVitePlugin } from '@sentry/vite-plugin'
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
    // Put the Sentry vite plugin after all other plugins
    sentryVitePlugin({
      org: 'collect-so',
      project: 'dashboard-frontend',
      // Auth tokens can be obtained from https://sentry.io/orgredirect/workspaces/:orgslug/settings/auth-tokens/
      authToken:
        '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'
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
    exclude: ['@collect.so/javascript-sdk']
  },
  server: {
    port: 3005
  }
})
