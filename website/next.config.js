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
