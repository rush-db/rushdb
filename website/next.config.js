const { version } = require('./package.json')

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  productionBrowserSourceMaps: true,
  images: {
    unoptimized: true
  },
  env: {
    NEXT_PUBLIC_APP_VERSION: version
  },
  // Remove standalone output for Amplify - it uses its own serverless setup
  // output: 'standalone',

  // Enable experimental ISR features for better Amplify compatibility
  experimental: {
    isrMemoryCacheSize: 0 // Disable in-memory cache for better ISR behavior on Amplify
  },
  // compiler: {
  //   styledJsx: false
  // },
  turbopack: {
    rules: {
      '*.svg': {
        loaders: ['@svgr/webpack'],
        as: '*.js'
      }
    }
  }
}

module.exports = nextConfig
